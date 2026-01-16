use tokio::sync::mpsc;
use std::time::{SystemTime, Duration};
use crate::market_data::types::MarketEvent;
use crate::state::market;
use crate::state::market_cache::{MarketCache, MarketKey, insert};
use std::collections::HashMap;
use crate::market_data::types::Venue;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn run_market_worker(mut rx: mpsc::Receiver<MarketEvent>, handle: Arc<RwLock<MarketCache>>) -> anyhow::Result<()> {

    while let Some(mut event) = rx.recv().await {
        // Update marketcache logic here
        let marketKey = MarketKey(event.venue.clone(), event.market_id.clone());
        let state = market::MarketState {
            last_event: Some(event.clone()),
            best_bid: event.best_bid,
            best_ask: event.best_ask,
            volume24h: event.volume24h,
        };

        insert(&handle, marketKey, state).await;
    }

    ;
    Ok(())
}