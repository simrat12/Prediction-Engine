mod clob;
mod types;
mod ws;

pub use types::{MarketInfo, MarketMap, TokenToMarket};

use clob::fetch_prices;
use types::{EligibleMarket, try_parse_eligible};
use ws::run_ws_loop;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{info, warn, debug, error};
use futures::StreamExt;

use polymarket_rs::client::GammaClient;
use polymarket_rs::request::GammaMarketParams;
use polymarket_rs::ClobClient;
use rust_decimal::prelude::ToPrimitive;

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use crate::metrics::prometheus::{record_adapter_event, record_adapter_latency};

// ── Public handle returned to main ───────────────────────────────────────────

/// Returned by [`init_polymarket_adapter`].
/// Holds the market metadata maps and the background task handle.
pub struct PolymarketAdapterHandle {
    /// All eligible markets indexed by market_id.
    pub market_map: MarketMap,
    /// Reverse lookup: token_id → market_id.
    pub token_to_market: Arc<TokenToMarket>,
    /// Background task running the adapter loop.
    pub handle: JoinHandle<anyhow::Result<()>>,
}

// ── Initialisation ────────────────────────────────────────────────────────────

/// Initialise the Polymarket adapter.
///
/// 1. Fetches all active markets from the Gamma API.
/// 2. Filters down to eligible binary CLOB markets (volume + liquidity thresholds).
/// 3. Builds `market_map` and `token_to_market` lookup tables.
/// 4. Spawns a background task that:
///    a. Fires an initial CLOB REST price fetch for every token (parallel, 10 at a time).
///    b. Connects to the WebSocket and streams live order book updates indefinitely.
///
/// Market events are sent over `tx` and consumed downstream by the router.
pub async fn init_polymarket_adapter(
    tx: mpsc::Sender<MarketEvent>,
) -> anyhow::Result<PolymarketAdapterHandle> {
    let gamma = GammaClient::new("https://gamma-api.polymarket.com");
    let clob = Arc::new(ClobClient::new("https://clob.polymarket.com"));

    // ── Step 1: Fetch and filter markets ─────────────────────────────────────
    let params = GammaMarketParams::new()
        .with_active(true)
        .with_closed(false)
        .with_archived(false)
        .with_limit(500);

    let raw_markets = gamma.get_markets(Some(params)).await?;
    info!(total = raw_markets.len(), "fetched markets from Gamma API");

    let eligible: Vec<EligibleMarket> = raw_markets
        .iter()
        .filter_map(try_parse_eligible)
        .collect();

    info!(count = eligible.len(), "eligible binary CLOB-tradable markets");

    // ── Step 2: Build lookup tables ───────────────────────────────────────────
    let mut token_ids: Vec<String> = Vec::with_capacity(eligible.len() * 2);
    let mut market_map: MarketMap = HashMap::with_capacity(eligible.len());
    let mut token_to_market: TokenToMarket = HashMap::with_capacity(eligible.len() * 2);

    for em in &eligible {
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

    // ── Step 3: Spawn background adapter task ─────────────────────────────────
    let handle = tokio::spawn(run_adapter_loop(
        tx,
        clob,
        Arc::clone(&token_to_market),
        eligible,
        token_ids,
    ));

    Ok(PolymarketAdapterHandle { market_map, token_to_market, handle })
}

// ── Background adapter loop ───────────────────────────────────────────────────

/// Orchestrates the initial price fetch and the live WebSocket stream.
///
/// The WebSocket is started immediately so we don't miss any events during the
/// (potentially slow) initial CLOB REST price fetch.
async fn run_adapter_loop(
    tx: mpsc::Sender<MarketEvent>,
    clob: Arc<ClobClient>,
    token_to_market: Arc<TokenToMarket>,
    eligible: Vec<EligibleMarket>,
    token_ids: Vec<String>,
) -> anyhow::Result<()> {
    // Start the WS loop immediately in its own task so we don't miss events
    // while the initial CLOB REST fetch is in progress.
    let ws_handle = tokio::spawn(run_ws_loop(
        tx.clone(),
        token_ids,
        Arc::clone(&token_to_market),
    ));

    // Initial CLOB REST price fetch — run up to 10 requests concurrently.
    // This seeds the market cache before the first WS event arrives.
    let price_futures = eligible.into_iter().map(|em| {
        let clob = Arc::clone(&clob);
        let tx = tx.clone();
        async move { fetch_and_emit_heartbeats(clob, tx, em).await }
    });

    futures::stream::iter(price_futures)
        .buffer_unordered(10)
        .for_each(|_| async {})
        .await;

    info!("initial CLOB price fetch complete");

    if let Err(e) = ws_handle.await {
        error!(error = %e, "WebSocket task panicked");
    }

    Ok(())
}

/// Fetch CLOB prices for both tokens of a market and emit `Heartbeat` events.
async fn fetch_and_emit_heartbeats(
    clob: Arc<ClobClient>,
    tx: mpsc::Sender<MarketEvent>,
    em: EligibleMarket,
) {
    for token_id in &em.token_ids {
        let start = Instant::now();
        let prices = fetch_prices(&clob, token_id, &em.market_id).await;
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        record_adapter_event("Polymarket", "heartbeat");
        record_adapter_latency("Polymarket", "heartbeat", latency_ms);

        // CLOB API semantics:  side=BUY → best ask,  side=SELL → best bid
        let (best_bid, best_ask) = match prices {
            Some((buy_price, sell_price)) => (sell_price.to_f64(), buy_price.to_f64()),
            None => (None, None),
        };

        let event = MarketEvent {
            venue: Venue::Polymarket,
            kind: MarketEventKind::Heartbeat,
            market_id: em.market_id.clone(),
            token_id: token_id.clone(),
            ts_exchange_ms: Some(SystemTime::now()),
            ts_receive_ms: None,
            received_at: Instant::now(),
            volume24h: Some(em.volume),
            last_trade_price: em.last_trade_price,
            liquidity: em.liquidity,
            best_bid,
            best_ask,
        };

        if tx.send(event).await.is_err() {
            warn!("channel closed during initial heartbeat");
            return;
        }
    }
}
