#![allow(warnings)] 

mod config;
use std::sync::Arc;

pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
mod market_data;
mod state;
use tokio::sync::mpsc;
use market_data::router;
use market_data::types::Venue;
use state::market_cache::{MarketCache, MarketKey};
use tokio::sync::RwLock;
use market_data::adapters::polymarket;



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

    let state = state::market_cache::MarketCache::new();

    let handle = Arc::new(RwLock::new(state));

    let pm_handle = tokio::spawn(polymarket::run_polymarket_adapter(tx));
    let router_handle = tokio::spawn(router::run_router(rx, handle.clone()));

    // periodically print what’s in cache
    // loop {
    //     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    //     let guard = handle.read().await;
    //     let markets = guard.get_markets_by_venue(&Venue::Polymarket);
    //     println!("cached polymarket markets = {}", markets.len());
    // }


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
                _ => warn!("router task panicked"),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl-C, shutting down");
        }
    }

    Ok(())
}
