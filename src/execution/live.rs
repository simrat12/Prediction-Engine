use std::str::FromStr;
use async_trait::async_trait;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use tracing::{info, warn};
use polymarket_rs::{
    AuthenticatedClient, OrderBuilder, PrivateKeySigner,
    SignatureType, TradingClient,
};
use polymarket_rs::types::{OrderArgs, CreateOrderOptions, OrderType};

use crate::market_data::types::Side as OurSide;
use super::traits::{ExecutionEngine, ExecutionIntent, ExecutionReport, LegFillStatus};
use std::time::Instant;

const CLOB_HOST: &str = "https://clob.polymarket.com";
const POLYGON_CHAIN_ID: u64 = 137;

pub struct LiveExecutor {
    client: TradingClient,
    tick_size: Decimal,
}

impl LiveExecutor {
    pub fn new(client: TradingClient, tick_size: Decimal) -> Self {
        Self { client, tick_size }
    }
}

pub async fn load_trading_client() -> anyhow::Result<TradingClient> {
    dotenvy::dotenv().ok();

    let private_key = std::env::var("PRIVATE_KEY")
        .expect("PRIVATE_KEY environment variable not set");
    let signer = PrivateKeySigner::from_str(&private_key)
        .expect("invalid PRIVATE_KEY");

    let auth_client = AuthenticatedClient::new(
        CLOB_HOST,
        signer.clone(),
        POLYGON_CHAIN_ID,
        None,
        None,
    );
    let api_creds = auth_client.create_or_derive_api_key().await?;

    let order_builder = OrderBuilder::new(
        signer.clone(),
        Some(SignatureType::Eoa),
        None,
    );

    let trading_client = TradingClient::new(
        CLOB_HOST,
        signer,
        POLYGON_CHAIN_ID,
        api_creds,
        order_builder,
    );

    Ok(trading_client)
}

fn convert_side(side: &OurSide) -> polymarket_rs::Side {
    match side {
        OurSide::Buy => polymarket_rs::Side::Buy,
        OurSide::Sell => polymarket_rs::Side::Sell,
    }
}

#[async_trait]
impl ExecutionEngine for LiveExecutor {
    async fn execute(&self, intent: ExecutionIntent) -> ExecutionReport {
        let mut leg_results = Vec::with_capacity(intent.legs.len());

        for (i, leg) in intent.legs.iter().enumerate() {
            let price = Decimal::try_from(leg.price).unwrap_or_default();
            let size = Decimal::try_from(leg.size).unwrap_or_default();

            let order_args = OrderArgs {
                token_id: leg.token_id.clone(),
                price,
                size,
                side: convert_side(&leg.side),
            };

            let options = CreateOrderOptions {
                tick_size: Some(self.tick_size),
                neg_risk: Some(intent.neg_risk),
            };

            let signed_order = match self.client.create_order(&order_args, None, None, options) {
                Ok(order) => order,
                Err(e) => {
                    warn!(
                        leg = i,
                        token_id = %leg.token_id,
                        error = %e,
                        "failed to create order"
                    );
                    leg_results.push(LegFillStatus::Rejected {
                        reason: format!("create_order failed: {e}"),
                    });
                    // Mark remaining legs as not attempted
                    for _ in (i + 1)..intent.legs.len() {
                        leg_results.push(LegFillStatus::NotAttempted);
                    }
                    break;
                }
            };

            match self.client.post_order(signed_order, OrderType::Fok).await {
                Ok(resp) if resp.success => {
                    info!(
                        order_id = %resp.order_id,
                        token_id = %leg.token_id,
                        side = ?leg.side,
                        price = %price,
                        size = %size,
                        "LIVE FILL"
                    );
                    leg_results.push(LegFillStatus::Filled {
                        order_id: resp.order_id.to_string(),
                        avg_price: price.to_f64().unwrap_or(leg.price),
                        filled_size: size.to_f64().unwrap_or(leg.size),
                    });
                }
                Ok(resp) => {
                    warn!(
                        leg = i,
                        token_id = %leg.token_id,
                        error_msg = %resp.error_msg,
                        status = %resp.status,
                        "order rejected by CLOB"
                    );
                    leg_results.push(LegFillStatus::Rejected {
                        reason: resp.error_msg,
                    });
                    for _ in (i + 1)..intent.legs.len() {
                        leg_results.push(LegFillStatus::NotAttempted);
                    }
                    break;
                }
                Err(e) => {
                    warn!(
                        leg = i,
                        token_id = %leg.token_id,
                        error = %e,
                        "post_order failed"
                    );
                    leg_results.push(LegFillStatus::Rejected {
                        reason: format!("post_order failed: {e}"),
                    });
                    for _ in (i + 1)..intent.legs.len() {
                        leg_results.push(LegFillStatus::NotAttempted);
                    }
                    break;
                }
            }
        }

        ExecutionReport {
            market_id: intent.market_id,
            strategy_name: intent.strategy_name,
            leg_results,
            completed_at: Instant::now(),
        }
    }
}
