use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use std::collections::HashMap;
use crate::market_data::types::{MarketEvent, Venue};
use crate::market_data::market_worker::run_market_worker;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::state::market_cache::MarketCache;

pub async fn run_router(mut rx: mpsc::Receiver<MarketEvent>, handle: Arc<RwLock<MarketCache>>) -> anyhow::Result<()> {
    let mut lanes: HashMap<Venue, mpsc::Sender<MarketEvent>> = HashMap::new();

    tokio::spawn(async move {

        while let Some(mut event) = rx.recv().await {

            if !lanes.contains_key(&event.venue) {
                let (lane_tx, mut lane_rx) = mpsc::channel(100);
                tokio::spawn(run_market_worker(lane_rx, handle.clone()));
                lanes.insert(event.venue.clone(), lane_tx);
            }

            lanes[&event.venue].send(event).await.unwrap();

        }
    }).await?;

    Ok(())
}
