#![allow(warnings)]

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn, debug, error};

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use crate::metrics::prometheus::{record_adapter_event, record_adapter_latency};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use futures::stream::{self, StreamExt as FuturesStreamExt};
use polymarket_rs::client::GammaClient;
use polymarket_rs::types::{GammaMarket, WsEvent};
use polymarket_rs::request::GammaMarketParams;
use polymarket_rs::ClobClient;
use polymarket_rs::types::TokenId;
use polymarket_rs::Side;
use polymarket_rs::websocket::MarketWsClient;
use polymarket_rs::StreamExt;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Metadata for a binary prediction market.
#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub market_id: String,
    pub question: String,
    pub yes_token_id: String,
    pub no_token_id: String,
    pub neg_risk: bool,
}

/// market_id → MarketInfo
pub type MarketMap = HashMap<String, MarketInfo>;

/// token_id → market_id (reverse lookup)
pub type TokenToMarket = HashMap<String, String>;

struct EligibleMarket {
    market_id: String,
    question: String,
    token_ids: Vec<String>,
    volume: f64,
    last_trade_price: Option<f64>,
    liquidity: Option<f64>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
    neg_risk: bool,
}

fn try_parse_eligible(m: &GammaMarket) -> Option<EligibleMarket> {
    if !m.active || m.closed || m.archived {
        return None;
    }

    let raw_ids = m.clob_token_ids.as_deref()?;
    let raw_prices = m.outcome_prices.as_deref()?;

    let prices: Vec<f64> = serde_json::from_str::<Vec<String>>(raw_prices)
        .ok()
        .map(|v| {
            v.into_iter()
                .filter_map(|p| p.parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default();

    if !prices.iter().any(|p| *p > 1e-6) {
        return None;
    }

    let ids: Vec<String> = serde_json::from_str(raw_ids).ok()?;

    if ids.len() != 2 {
        return None;
    }

    let volume24h = m.volume24hr.unwrap_or(0.0);
    if volume24h < 100_000.0 {
        return None;
    }

    let liquidity_val = m.liquidity_num.unwrap_or(0.0);
    if liquidity_val < 10_000.0 {
        return None;
    }

    let volume = m
        .volume
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0);

    let neg_risk = false;

    Some(EligibleMarket {
        market_id: m.id.clone(),
        question: m.question.clone(),
        token_ids: ids,
        volume,
        last_trade_price: m.last_trade_price,
        liquidity: m.liquidity.as_ref().and_then(|l| l.parse::<f64>().ok()),
        best_bid: m.best_bid,
        best_ask: m.best_ask,
        neg_risk,
    })
}

async fn fetch_prices(
    clob_client: &ClobClient,
    token_id: &str,
    market_id: &str,
) -> Option<(Decimal, Decimal)> {
    let tid: TokenId = TokenId::from(token_id.to_owned());
    let (buy_res, sell_res) = tokio::join!(
        clob_client.get_price(&tid, Side::Buy),
        clob_client.get_price(&tid, Side::Sell),
    );

    let buy_price = match buy_res {
        Ok(p) => p.price,
        Err(e) => {
            warn!(market_id, error = %e, "failed to fetch buy price");
            return None;
        }
    };
    let sell_price = match sell_res {
        Ok(p) => p.price,
        Err(e) => {
            warn!(market_id, error = %e, "failed to fetch sell price");
            return None;
        }
    };

    Some((buy_price, sell_price))
}

const MAX_WS_RECONNECT_ATTEMPTS: u32 = 10;
const INITIAL_BACKOFF_MS: u64 = 500;
const MAX_BACKOFF_MS: u64 = 30_000;

pub struct PolymarketAdapterHandle {
    pub market_map: MarketMap,
    pub token_to_market: Arc<TokenToMarket>,
    pub handle: JoinHandle<anyhow::Result<()>>,
}

pub async fn init_polymarket_adapter(
    tx: mpsc::Sender<MarketEvent>,
) -> anyhow::Result<PolymarketAdapterHandle> {
    let client = GammaClient::new("https://gamma-api.polymarket.com");
    let clob_client = Arc::new(ClobClient::new("https://clob.polymarket.com"));

    let params = GammaMarketParams::new()
        .with_active(true)
        .with_closed(false)
        .with_archived(false)
        .with_limit(500);

    let markets = client.get_markets(Some(params)).await?;
    info!(total = markets.len(), "fetched markets from Gamma API");

    let eligible: Vec<EligibleMarket> = markets
        .iter()
        .filter_map(try_parse_eligible)
        .collect();

    info!(count = eligible.len(), "eligible binary CLOB-tradable markets");

    let mut token_ids: Vec<String> = Vec::with_capacity(eligible.len() * 2);
    let mut market_map: MarketMap = HashMap::with_capacity(eligible.len());
    let mut token_to_market: TokenToMarket = HashMap::with_capacity(eligible.len() * 2);

    for em in eligible.iter() {
        debug!(market_id = %em.market_id, volume = em.volume, "eligible market");

        for tid in &em.token_ids {
            token_ids.push(tid.clone());
            token_to_market.insert(tid.clone(), em.market_id.clone());
        }

        market_map.insert(em.market_id.clone(), MarketInfo {
            market_id: em.market_id.clone(),
            question: em.question.clone(),
            yes_token_id: em.token_ids[0].clone(),
            no_token_id: em.token_ids[1].clone(),
            neg_risk: em.neg_risk,
        });
    }

    let token_to_market = Arc::new(token_to_market);

    let handle = tokio::spawn(run_adapter_loop(
        tx, clob_client, Arc::clone(&token_to_market), eligible, token_ids,
    ));

    Ok(PolymarketAdapterHandle {
        market_map,
        token_to_market,
        handle,
    })
}

async fn run_adapter_loop(
    tx: mpsc::Sender<MarketEvent>,
    clob_client: Arc<ClobClient>,
    token_to_market: Arc<TokenToMarket>,
    eligible: Vec<EligibleMarket>,
    token_ids: Vec<String>,
) -> anyhow::Result<()> {
    let ws_token_ids = token_ids.clone();
    let ws_tx = tx.clone();
    let ws_lookup = Arc::clone(&token_to_market);

    let ws_handle = tokio::spawn(async move {
        run_ws_loop(ws_tx, ws_token_ids, ws_lookup).await;
    });

    let price_futures: Vec<_> = eligible.into_iter().map(|em| {
        let clob = Arc::clone(&clob_client);
        let market_id = em.market_id;
        let token_ids_pair = em.token_ids;
        let volume = em.volume;
        let last_trade_price = em.last_trade_price;
        let liquidity = em.liquidity;
        let best_bid = em.best_bid;
        let best_ask = em.best_ask;
        let tx = tx.clone();

        async move {
            for token_id in &token_ids_pair {
                let start = Instant::now();
                let _prices = fetch_prices(&clob, token_id, &market_id).await;
                let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

                record_adapter_event("Polymarket", "heartbeat");
                record_adapter_latency("Polymarket", "heartbeat", latency_ms);

                let event = MarketEvent {
                    venue: Venue::Polymarket,
                    kind: MarketEventKind::Heartbeat,
                    market_id: market_id.clone(),
                    token_id: token_id.clone(),
                    ts_exchange_ms: Some(SystemTime::now()),
                    ts_receive_ms: None,
                    received_at: Instant::now(),
                    volume24h: Some(volume),
                    last_trade_price,
                    liquidity,
                    best_bid,
                    best_ask,
                };

                if tx.send(event).await.is_err() {
                    warn!("channel closed while sending heartbeat");
                    return;
                }
            }
        }
    }).collect();

    futures::stream::iter(price_futures)
        .buffer_unordered(10)
        .for_each(|_| async {})
        .await;

    info!("initial price fetch complete, WebSocket running");

    if let Err(e) = ws_handle.await {
        error!(error = %e, "WebSocket task panicked");
    }

    Ok(())
}

const WS_LOG_INTERVAL: Duration = Duration::from_secs(30);

async fn run_ws_loop(
    tx: mpsc::Sender<MarketEvent>,
    token_ids: Vec<String>,
    token_to_market: Arc<TokenToMarket>,
) {
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        info!(attempt, "connecting to Polymarket WebSocket");

        let ws_client = MarketWsClient::new();
        let stream_result = ws_client.subscribe(token_ids.clone()).await;

        let mut stream = match stream_result {
            Ok(s) => {
                info!("WebSocket connected, subscribed to {} tokens", token_ids.len());
                attempt = 0;
                s
            }
            Err(e) => {
                if attempt >= MAX_WS_RECONNECT_ATTEMPTS {
                    error!(
                        error = %e,
                        attempts = attempt,
                        "max WebSocket reconnection attempts reached, giving up"
                    );
                    return;
                }
                let backoff = std::cmp::min(
                    INITIAL_BACKOFF_MS * 2u64.saturating_pow(attempt - 1),
                    MAX_BACKOFF_MS,
                );
                warn!(
                    error = %e,
                    attempt,
                    backoff_ms = backoff,
                    "WebSocket connection failed, retrying"
                );
                tokio::time::sleep(Duration::from_millis(backoff)).await;
                continue;
            }
        };

        let mut last_log = Instant::now();
        let mut interval_price_changes: u64 = 0;
        let mut interval_unknown: u64 = 0;

        while let Some(message) = stream.next().await {
            match message {
                Ok(WsEvent::PriceChange(price_change)) => {
                    let received_at = Instant::now();

                    let Some(first_pc) = price_change.price_changes.first() else {
                        continue;
                    };
                    let asset_id = first_pc.asset_id.clone();
                    let Some(market_id) = token_to_market.get(&asset_id).cloned() else {
                        interval_unknown += 1;
                        debug!(asset_id = %asset_id, "unknown token id from WS");
                        continue;
                    };

                    interval_price_changes += 1;
                    record_adapter_event("Polymarket", "price_change");

                    debug!(
                        asset_id = %asset_id,
                        market_id,
                        "received price change"
                    );

                    let event = MarketEvent {
                        venue: Venue::Polymarket,
                        kind: MarketEventKind::PriceChange,
                        market_id,
                        token_id: asset_id,
                        ts_exchange_ms: None,
                        ts_receive_ms: Some(SystemTime::now()),
                        received_at,
                        volume24h: None,
                        last_trade_price: None,
                        liquidity: None,
                        best_bid: price_change
                            .price_changes
                            .iter()
                            .find(|pc| pc.side == Side::Buy)
                            .and_then(|pc| pc.price.to_f64()),
                        best_ask: price_change
                            .price_changes
                            .iter()
                            .find(|pc| pc.side == Side::Sell)
                            .and_then(|pc| pc.price.to_f64()),
                    };

                    if tx.send(event).await.is_err() {
                        warn!("channel closed, stopping WebSocket loop");
                        return;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(error = %e, "WebSocket stream error");
                }
            }

            if last_log.elapsed() >= WS_LOG_INTERVAL {
                info!(
                    price_changes = interval_price_changes,
                    unknown = interval_unknown,
                    "WebSocket activity (last {}s)", WS_LOG_INTERVAL.as_secs()
                );
                interval_price_changes = 0;
                interval_unknown = 0;
                last_log = Instant::now();
            }
        }

        if attempt >= MAX_WS_RECONNECT_ATTEMPTS {
            error!(
                attempts = attempt,
                "max WebSocket reconnection attempts reached, giving up"
            );
            return;
        }

        let backoff = std::cmp::min(
            INITIAL_BACKOFF_MS * 2u64.saturating_pow(attempt),
            MAX_BACKOFF_MS,
        );
        warn!(
            attempt,
            backoff_ms = backoff,
            "WebSocket stream ended, reconnecting"
        );
        tokio::time::sleep(Duration::from_millis(backoff)).await;
    }
}
