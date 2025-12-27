mod config;
pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
mod market_data;
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

    market_data::adapters::polymarket::run_polymarket_adapter(tx).await?;

    market_data::router::run_router(rx).await?;

    Ok(())
}
