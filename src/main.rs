mod config;
use std::sync::Arc;

pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
mod market_data;
mod state;
use tokio::sync::mpsc;


fn init_tracing() {

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    // tiny async sanity check
    let handle = tokio::spawn(async {
        warn!("tokio task ran");
        42u32
    });

    let x = handle.await?;
    info!(x, "done");

    let (tx, mut rx) = mpsc::channel(100);

    let handle = Arc::new(tokio::sync::RwLock::new(state::market_cache::MarketCache::new()));

    tokio::spawn(market_data::adapters::polymarket::run_polymarket_adapter(tx));

    market_data::router::run_router(rx, handle).await?;


    Ok(())
}
