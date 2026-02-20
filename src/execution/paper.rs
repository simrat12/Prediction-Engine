use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tracing::info;

use super::traits::{ExecutionEngine, ExecutionIntent, ExecutionReport, LegFillStatus};

pub struct PaperExecutor {
    next_order_id: AtomicU64,
}

impl PaperExecutor {
    pub fn new() -> Self {
        Self {
            next_order_id: AtomicU64::new(1),
        }
    }
}

#[async_trait]
impl ExecutionEngine for PaperExecutor {
    async fn execute(&self, intent: ExecutionIntent) -> ExecutionReport {
        let leg_results: Vec<LegFillStatus> = intent
            .legs
            .iter()
            .map(|leg| {
                let order_id = self.next_order_id.fetch_add(1, Ordering::Relaxed);

                info!(
                    order_id,
                    token_id = %leg.token_id,
                    side = ?leg.side,
                    price = leg.price,
                    size = leg.size,
                    market_id = %intent.market_id,
                    "PAPER FILL"
                );

                LegFillStatus::Filled {
                    order_id: order_id.to_string(),
                    avg_price: leg.price,
                    filled_size: leg.size,
                }
            })
            .collect();

        ExecutionReport {
            market_id: intent.market_id,
            strategy_name: intent.strategy_name,
            leg_results,
            completed_at: Instant::now(),
        }
    }
}
