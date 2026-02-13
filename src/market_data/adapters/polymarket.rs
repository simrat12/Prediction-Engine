#![allow(warnings)]

use tokio::sync::mpsc;
use tracing::{info, warn, debug, error};

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
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
use tokio::sync::Mutex;
use crate::metrics::prometheus::Metrics;

/// Pre-parsed market data to avoid redundant JSON parsing.
struct EligibleMarket {
    market_id: String,
    question: String,
    token_ids: Vec<String>,
    volume: f64,
    last_trade_price: Option<f64>,
    liquidity: Option<f64>,
    best_bid: Option<f64>,
    best_ask: Option<f64>,
}

/// Parse and validate a GammaMarket for CLOB tradability in a single pass.
/// Returns parsed data to avoid redundant JSON deserialization downstream.
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

    // Must have at least 1 non-zero price (liquidity signal)
    if !prices.iter().any(|p| *p > 1e-6) {
        return None;
    }

    let ids: Vec<String> = serde_json::from_str(raw_ids).ok()?;
    if ids.is_empty() {
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

    Some(EligibleMarket {
        market_id: m.id.clone(),
        question: m.question.clone(),
        token_ids: ids,
        volume,
        last_trade_price: m.last_trade_price,
        liquidity: m.liquidity.as_ref().and_then(|l| l.parse::<f64>().ok()),
        best_bid: m.best_bid,
        best_ask: m.best_ask,
    })
}

/// Fetch buy + sell prices for a single token concurrently.
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

    if buy_price + sell_price > Decimal::from(1) {
        info!(
            market_id,
            %buy_price,
            %sell_price,
            "arbitrage opportunity detected"
        );
    }

    Some((buy_price, sell_price))
}

const MAX_WS_RECONNECT_ATTEMPTS: u32 = 10;
const INITIAL_BACKOFF_MS: u64 = 500;
const MAX_BACKOFF_MS: u64 = 30_000;

pub async fn run_polymarket_adapter(
    tx: mpsc::Sender<MarketEvent>,
    metrics: Arc<Mutex<Metrics>>,
) -> anyhow::Result<()> {
    let client = GammaClient::new("https://gamma-api.polymarket.com");
    let clob_client = Arc::new(ClobClient::new("https://clob.polymarket.com"));

    // ── 1. Fetch & filter markets ──────────────────────────────────────
    let params = GammaMarketParams::new()
        .with_active(true)
        .with_closed(false)
        .with_archived(false)
        .with_limit(500);

    let markets = client.get_markets(Some(params)).await?;
    info!(total = markets.len(), "fetched markets from Gamma API");

    // Single-pass parse + filter — no redundant JSON deserialization
    let eligible: Vec<EligibleMarket> = markets
        .iter()
        .filter_map(try_parse_eligible)
        .collect();

    info!(count = eligible.len(), "eligible CLOB-tradable markets");

    // ── 2. Build lookup tables ─────────────────────────────────────────
    let mut token_ids: Vec<String> = Vec::with_capacity(eligible.len() * 2);
    let mut clob_to_gamma: HashMap<String, String> =
        HashMap::with_capacity(eligible.len() * 2);
    let mut question_map: HashMap<String, String> =
        HashMap::with_capacity(eligible.len());

    for em in &eligible {
        debug!(market_id = %em.market_id, volume = em.volume, "eligible market");
        for tid in &em.token_ids {
            token_ids.push(tid.clone());
            clob_to_gamma.insert(tid.clone(), em.market_id.clone());
        }
        question_map.insert(em.market_id.clone(), em.question.clone());
    }

    // Make lookup table immutable + shareable for the WS loop
    let clob_to_gamma: Arc<HashMap<String, String>> = Arc::new(clob_to_gamma);

    // ── 3. Start WS subscription concurrently with initial price fetch ─
    //    This eliminates the blind window where we miss price changes.
    let ws_token_ids = token_ids.clone();
    let ws_tx = tx.clone();
    let ws_lookup = Arc::clone(&clob_to_gamma);
    let ws_metrics = Arc::clone(&metrics);

    let ws_handle = tokio::spawn(async move {
        run_ws_loop(ws_tx, ws_token_ids, ws_lookup, ws_metrics).await;
    });

    // ── 4. Fetch initial CLOB prices with buffered concurrency ─────────
    //    Up to 10 concurrent HTTP requests instead of sequential.
    let price_futures: Vec<_> = eligible.into_iter().map(|em| {
        let clob = Arc::clone(&clob_client);
        let metrics: Arc<Mutex<Metrics>> = Arc::clone(&metrics);
        let market_id = em.market_id;
        let first_token = em.token_ids.into_iter().next();
        let volume = em.volume;
        let last_trade_price = em.last_trade_price;
        let liquidity = em.liquidity;
        let best_bid = em.best_bid;
        let best_ask = em.best_ask;
        let tx = tx.clone();

        async move {
            let Some(token_id) = first_token else {
                warn!(market_id, "no token IDs found");
                return;
            };

            let start = std::time::Instant::now();

            // Fetch buy + sell concurrently for this market
            let _prices = fetch_prices(&clob, &token_id, &market_id).await;

            let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

            let event = MarketEvent {
                venue: Venue::Polymarket,
                kind: MarketEventKind::Heartbeat,
                market_id,
                ts_exchange_ms: Some(SystemTime::now()),
                ts_receive_ms: None,
                volume24h: Some(volume),
                last_trade_price,
                liquidity,
                best_bid,
                best_ask,
            };

            {
                let mut m = metrics.lock().await;
                m.increment_counter("Polymarket", "heartbeat");
                m.observe_latency("Polymarket", "heartbeat", latency_ms);
            }

            if tx.send(event).await.is_err() {
                warn!("channel closed while sending heartbeat");
            }
        }
    }).collect();

    // Buffer up to 10 concurrent price fetches to avoid hammering the API
    futures::stream::iter(price_futures)
        .buffer_unordered(10)
        .for_each(|_| async {})
        .await;

    info!("initial price fetch complete, WebSocket running");

    // Wait for the WS loop to finish (only on disconnect/error)
    if let Err(e) = ws_handle.await {
        error!(error = %e, "WebSocket task panicked");
    }

    Ok(())
}

/// How often to log a WebSocket summary at info level.
const WS_LOG_INTERVAL: Duration = Duration::from_secs(30);

/// WebSocket loop with automatic reconnection and exponential backoff.
async fn run_ws_loop(
    tx: mpsc::Sender<MarketEvent>,
    token_ids: Vec<String>,
    clob_to_gamma: Arc<HashMap<String, String>>,
    metrics: Arc<Mutex<Metrics>>,
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
                attempt = 0; // reset backoff on successful connection
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

        // Periodic summary counters
        let mut last_log = std::time::Instant::now();
        let mut interval_price_changes: u64 = 0;
        let mut interval_unknown: u64 = 0;

        // Process messages from the stream
        while let Some(message) = stream.next().await {
            match message {
                Ok(WsEvent::PriceChange(price_change)) => {

                    let Some(first_pc) = price_change.price_changes.first() else {
                        continue;
                    };
                    let Some(market_id) = clob_to_gamma.get(&first_pc.asset_id).cloned() else {
                        interval_unknown += 1;
                        debug!(asset_id = %first_pc.asset_id, "unknown token id from WS");
                        continue;
                    };

                    interval_price_changes += 1;

                    debug!(
                        asset_id = %first_pc.asset_id,
                        market_id,
                        "received price change"
                    );

                    let event = MarketEvent {
                        venue: Venue::Polymarket,
                        kind: MarketEventKind::PriceChange,
                        market_id,
                        ts_exchange_ms: None,
                        ts_receive_ms: Some(SystemTime::now()),
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

                    {
                        let mut m = metrics.lock().await;
                        m.increment_counter("Polymarket", "price_change");
                    }

                    if tx.send(event).await.is_err() {
                        warn!("channel closed, stopping WebSocket loop");
                        return;
                    }
                }
                Ok(_) => {
                    // Ignore non-price-change events
                }
                Err(e) => {
                    warn!(error = %e, "WebSocket stream error");
                }
            }

            // Periodic info-level summary
            if last_log.elapsed() >= WS_LOG_INTERVAL {
                info!(
                    price_changes = interval_price_changes,
                    unknown = interval_unknown,
                    "WebSocket activity (last {}s)", WS_LOG_INTERVAL.as_secs()
                );
                interval_price_changes = 0;
                interval_unknown = 0;
                last_log = std::time::Instant::now();
            }
        }

        // Stream ended — attempt reconnection
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
