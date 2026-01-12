use tokio::sync::mpsc;
use std::time::{SystemTime, Duration};
use crate::market_data::types::MarketEvent;
use crate::state::market;
use crate::state::market_cache::{MarketCache, MarketKey};
use std::collections::HashMap;
use crate::market_data::types::Venue;

pub async fn run_market_worker(mut rx: mpsc::Receiver<MarketEvent>) -> anyhow::Result<()> {

    while let Some(mut event) = rx.recv().await {
        // Update marketcache logic here
        let marketKey = MarketKey(event.venue.clone(), event.market_id.clone());

    }

    ;
    Ok(())
}