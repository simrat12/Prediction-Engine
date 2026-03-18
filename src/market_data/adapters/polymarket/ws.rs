use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc;
use tracing::{info, warn, debug, error};
use rust_decimal::prelude::ToPrimitive;
use polymarket_rs::types::WsEvent;
use polymarket_rs::websocket::MarketWsClient;
use polymarket_rs::StreamExt;

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use crate::metrics::prometheus::record_adapter_event;
use super::types::TokenToMarket;

// ── Reconnect policy ──────────────────────────────────────────────────────────

const MAX_RECONNECT_ATTEMPTS: u32 = 10;
/// Initial reconnect wait: 500 ms, doubling on each failure up to MAX_BACKOFF_MS.
const INITIAL_BACKOFF_MS: u64 = 500;
const MAX_BACKOFF_MS: u64 = 30_000;

/// How often to emit a summary log of WebSocket activity.
const LOG_INTERVAL: Duration = Duration::from_secs(30);

// ── Public entry point ────────────────────────────────────────────────────────

/// Run the Polymarket WebSocket loop forever, reconnecting on failure.
///
/// Subscribes to order book updates for all `token_ids` and converts each
/// incoming event into a `MarketEvent` sent over `tx`.
///
/// Two event types are handled:
///
/// - **`BookEvent`**: Full order book snapshot sent on (re)connection.
///   `bids[0]` = highest bid, `asks[0]` = lowest ask — used directly as
///   best_bid/best_ask and sent as a `Heartbeat` to seed the market cache.
///
/// - **`PriceChangeEvent`**: Incremental update to a price level.
///   Each entry carries its own `asset_id`, `best_bid`, and `best_ask`
///   reflecting the real top-of-book *after* this update. A single event
///   often covers both the YES and NO tokens of the same market, so we
///   process each entry independently.
pub(super) async fn run_ws_loop(
    tx: mpsc::Sender<MarketEvent>,
    token_ids: Vec<String>,
    token_to_market: Arc<TokenToMarket>,
) {
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        info!(attempt, "connecting to Polymarket WebSocket");

        let ws_client = MarketWsClient::new();

        let mut stream = match ws_client.subscribe(token_ids.clone()).await {
            Ok(s) => {
                info!(tokens = token_ids.len(), "WebSocket connected");
                attempt = 0; // reset on successful connection
                s
            }
            Err(e) => {
                if attempt >= MAX_RECONNECT_ATTEMPTS {
                    error!(error = %e, attempts = attempt, "max WS reconnect attempts reached");
                    return;
                }
                let backoff_ms = backoff_duration(attempt);
                warn!(error = %e, attempt, backoff_ms, "WS connection failed, retrying");
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                continue;
            }
        };

        // Counters for the periodic activity log.
        let mut last_log = Instant::now();
        let mut events_since_log: u64 = 0;
        let mut unknown_since_log: u64 = 0;

        while let Some(message) = stream.next().await {
            match message {
                Ok(WsEvent::Book(book)) => {
                    handle_book_event(&tx, &token_to_market, book, &mut unknown_since_log).await;
                }
                Ok(WsEvent::PriceChange(pc)) => {
                    handle_price_change(&tx, &token_to_market, pc, &mut events_since_log, &mut unknown_since_log).await;
                }
                Ok(_) => {} // LastTradePrice, TickSizeChange — not needed yet
                Err(e) => {
                    warn!(error = %e, "WebSocket stream error");
                }
            }

            if last_log.elapsed() >= LOG_INTERVAL {
                info!(
                    price_changes = events_since_log,
                    unknown_tokens = unknown_since_log,
                    "WS activity (last {}s)", LOG_INTERVAL.as_secs()
                );
                events_since_log = 0;
                unknown_since_log = 0;
                last_log = Instant::now();
            }
        }

        // Stream ended — reconnect.
        if attempt >= MAX_RECONNECT_ATTEMPTS {
            error!(attempts = attempt, "max WS reconnect attempts reached");
            return;
        }
        let backoff_ms = backoff_duration(attempt);
        warn!(attempt, backoff_ms, "WS stream ended, reconnecting");
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
    }
}

// ── Event handlers ────────────────────────────────────────────────────────────

/// Handle a full order book snapshot (`BookEvent`).
///
/// Sent by Polymarket on initial connection (and after reconnects) for each
/// subscribed token. `bids` are sorted highest-first, `asks` lowest-first,
/// so `bids[0]` and `asks[0]` give us the real top-of-book.
async fn handle_book_event(
    tx: &mpsc::Sender<MarketEvent>,
    token_to_market: &Arc<TokenToMarket>,
    book: polymarket_rs::types::BookEvent,
    unknown_count: &mut u64,
) {
    let Some(market_id) = token_to_market.get(&book.asset_id).cloned() else {
        *unknown_count += 1;
        debug!(asset_id = %book.asset_id, "book snapshot for unknown token");
        return;
    };

    let best_bid = book.bids.first().and_then(|pl| pl.price.to_f64());
    let best_ask = book.asks.first().and_then(|pl| pl.price.to_f64());

    record_adapter_event("Polymarket", "book_snapshot");

    debug!(
        asset_id = %book.asset_id,
        market_id,
        ?best_bid,
        ?best_ask,
        bid_levels = book.bids.len(),
        ask_levels = book.asks.len(),
        "book snapshot received"
    );

    let event = MarketEvent {
        venue: Venue::Polymarket,
        kind: MarketEventKind::Heartbeat,
        market_id,
        token_id: book.asset_id,
        ts_exchange_ms: None,
        ts_receive_ms: Some(SystemTime::now()),
        received_at: Instant::now(),
        volume24h: None,
        last_trade_price: None,
        liquidity: None,
        best_bid,
        best_ask,
    };

    if tx.send(event).await.is_err() {
        warn!("channel closed during book snapshot");
    }
}

/// Handle an incremental order book update (`PriceChangeEvent`).
///
/// ## Why we process each entry separately
///
/// A single `PriceChangeEvent` commonly contains entries for **both** the YES
/// and NO tokens of the same market. Polymarket's CLOB is unified: placing a
/// bid to buy YES at price X automatically mirrors as an ask to sell NO at
/// (1 - X). The WS reports both sides, each with its own `asset_id`.
///
/// Each entry also carries `best_bid` and `best_ask` — the real top-of-book
/// values **after** this level change — so we use those directly rather than
/// trying to infer the spread from the changed price level.
async fn handle_price_change(
    tx: &mpsc::Sender<MarketEvent>,
    token_to_market: &Arc<TokenToMarket>,
    pc_event: polymarket_rs::types::PriceChangeEvent,
    event_count: &mut u64,
    unknown_count: &mut u64,
) {
    let received_at = Instant::now();
    let now = SystemTime::now();

    for pc in &pc_event.price_changes {
        let Some(market_id) = token_to_market.get(&pc.asset_id).cloned() else {
            *unknown_count += 1;
            debug!(asset_id = %pc.asset_id, "price change for unknown token");
            continue;
        };

        let best_bid = pc.best_bid.and_then(|d| d.to_f64());
        let best_ask = pc.best_ask.and_then(|d| d.to_f64());

        *event_count += 1;
        record_adapter_event("Polymarket", "price_change");

        debug!(
            asset_id = %pc.asset_id,
            market_id,
            side = ?pc.side,
            changed_price = %pc.price,
            changed_size = %pc.size,
            ?best_bid,
            ?best_ask,
            "price change received"
        );

        let event = MarketEvent {
            venue: Venue::Polymarket,
            kind: MarketEventKind::PriceChange,
            market_id,
            token_id: pc.asset_id.clone(),
            ts_exchange_ms: None,
            ts_receive_ms: Some(now),
            received_at,
            volume24h: None,
            last_trade_price: None,
            liquidity: None,
            best_bid,
            best_ask,
        };

        if tx.send(event).await.is_err() {
            warn!("channel closed during price change");
            return;
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Exponential backoff capped at `MAX_BACKOFF_MS`.
fn backoff_duration(attempt: u32) -> u64 {
    (INITIAL_BACKOFF_MS * 2u64.saturating_pow(attempt.saturating_sub(1))).min(MAX_BACKOFF_MS)
}
