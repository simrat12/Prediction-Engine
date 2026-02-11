#![allow(warnings)]

mod config;

pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
mod market_data;
mod state;
use tokio::sync::mpsc;
use market_data::router;
use market_data::types::Venue;
use state::market_cache::MarketCache;
use market_data::adapters::polymarket;
use metrics::prometheus::Metrics;
mod metrics;

/// Main adapter→router channel buffer.
/// Sized to absorb WebSocket bursts without back-pressuring the adapter.
const ADAPTER_CHANNEL_BUFFER: usize = 4_096;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    metrics::init_metrics();

    info!("prediction-engine starting");

    let (tx, rx) = mpsc::channel(ADAPTER_CHANNEL_BUFFER);

    // DashMap-backed cache — cheap to clone (just an Arc bump)
    let cache = MarketCache::new();

    let pm_handle = tokio::spawn(polymarket::run_polymarket_adapter(tx));
    let router_handle = tokio::spawn(router::run_router(rx, cache.clone()));

    tokio::select! {
        res = pm_handle => {
            match res {
                Ok(Ok(())) => warn!("polymarket adapter exited"),
                Ok(Err(err)) => warn!(error = %err, "polymarket adapter returned error"),
                Err(err) => warn!(error = %err, "polymarket adapter task panicked"),
            }
        }
        res = router_handle => {
            match res {
                Ok(Ok(())) => warn!("router task exited"),
                Ok(Err(err)) => warn!(error = %err, "router task returned error"),
                Err(err) => warn!(error = %err, "router task panicked"),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl-C, shutting down");
        }
    }

    Ok(())
}
