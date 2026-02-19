#![allow(warnings)]

use tokio::sync::mpsc;
use tracing::{info, warn};
use std::collections::HashMap;
use crate::market_data::types::{MarketEvent, Venue};
use crate::market_data::market_worker::run_market_worker;
use crate::state::market_cache::{MarketCache, MarketKey};

/// Per-venue lane buffer size.
const LANE_BUFFER: usize = 1_024;

pub async fn run_router(
    mut rx: mpsc::Receiver<MarketEvent>,
    handle: MarketCache,
    notify_tx: mpsc::Sender<MarketKey>,
) -> anyhow::Result<()> {
    let mut lanes: HashMap<Venue, mpsc::Sender<MarketEvent>> = HashMap::new();

    while let Some(event) = rx.recv().await {
        if !lanes.contains_key(&event.venue) {
            let (lane_tx, lane_rx) = mpsc::channel(LANE_BUFFER);
            info!(venue = ?event.venue, "spawning market worker");
            tokio::spawn(run_market_worker(lane_rx, handle.clone(), notify_tx.clone()));
            lanes.insert(event.venue.clone(), lane_tx);
        }

        if let Some(lane) = lanes.get(&event.venue) {
            if lane.send(event).await.is_err() {
                warn!("venue lane closed unexpectedly");
            }
        }
    }

    Ok(())
}
