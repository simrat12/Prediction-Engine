#![allow(warnings)]

use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{debug, warn};
use crate::market_data::types::MarketEvent;
use crate::state::market::MarketState;
use crate::state::market_cache::{MarketCache, MarketKey, insert};

/// Notification payload sent to the strategy engine.
/// Carries the WS receive timestamp for end-to-end latency measurement.
pub type Notification = (MarketKey, Instant);

pub async fn run_market_worker(
    mut rx: mpsc::Receiver<MarketEvent>,
    handle: MarketCache,
    notify_tx: mpsc::Sender<Notification>,
) -> anyhow::Result<()> {
    while let Some(event) = rx.recv().await {
        let key = MarketKey(event.venue.clone(), event.token_id.clone());
        let received_at = event.received_at;

        let state = MarketState {
            best_bid: event.best_bid,
            best_ask: event.best_ask,
            volume24h: event.volume24h,
        };

        debug!(
            token_id = %event.token_id,
            ?state,
            "updating cache"
        );

        insert(&handle, key.clone(), state);

        // Notify strategy engine — non-blocking so the data path
        // never stalls on a slow strategy consumer.
        let _ = notify_tx.try_send((key, received_at));
    }

    Ok(())
}
