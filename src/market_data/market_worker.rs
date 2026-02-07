#![allow(warnings)]

use tokio::sync::mpsc;
use tracing::{debug, warn};
use crate::market_data::types::MarketEvent;
use crate::state::market::MarketState;
use crate::state::market_cache::{MarketCache, MarketKey, insert};

pub async fn run_market_worker(mut rx: mpsc::Receiver<MarketEvent>, handle: MarketCache) -> anyhow::Result<()> {
    while let Some(event) = rx.recv().await {
        let key = MarketKey(event.venue.clone(), event.market_id.clone());

        // Build state directly from event fields — no redundant full-event clone
        let state = MarketState {
            best_bid: event.best_bid,
            best_ask: event.best_ask,
            volume24h: event.volume24h,
        };

        debug!(
            market_id = %event.market_id,
            ?state,
            "updating cache"
        );

        // Synchronous insert — DashMap handles concurrency internally
        insert(&handle, key, state);
    }

    Ok(())
}
