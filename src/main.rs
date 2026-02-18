#![allow(warnings)]

mod config;

pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
use tokio::sync::mpsc;
use std::sync::Arc;
use tokio::sync::Mutex;
use prediction_engine::market_data::router;
use prediction_engine::market_data::types::Venue;
use prediction_engine::state::market_cache::{MarketCache, MarketKey};
use prediction_engine::market_data::adapters::polymarket;
use prediction_engine::metrics::prometheus::Metrics;
use prediction_engine::strategy;
use prediction_engine::strategy::traits::TradeSignal;
use prediction_engine::strategy::arbitrage::ArbitrageStrategy;

const ADAPTER_CHANNEL_BUFFER: usize = 4_096;
const NOTIFY_CHANNEL_BUFFER: usize = 512;
const SIGNAL_CHANNEL_BUFFER: usize = 64;

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    prediction_engine::metrics::init_metrics();

    info!("prediction-engine starting");

    let (tx, rx) = mpsc::channel(ADAPTER_CHANNEL_BUFFER);

    // DashMap-backed cache — cheap to clone (just an Arc bump)
    let cache = MarketCache::new();

    let metrics = Arc::new(Mutex::new(Metrics::new()));

    // MarketWorker → StrategyEngine notification channel
    let (notify_tx, notify_rx) = mpsc::channel::<MarketKey>(NOTIFY_CHANNEL_BUFFER);

    // StrategyEngine → signal consumer channel
    let (signal_tx, signal_rx) = mpsc::channel::<TradeSignal>(SIGNAL_CHANNEL_BUFFER);

    let strategies: Vec<Box<dyn strategy::traits::Strategy>> = vec![
        Box::new(ArbitrageStrategy::new(0.01)),
    ];

    let pm_handle = tokio::spawn(polymarket::run_polymarket_adapter(tx, Arc::clone(&metrics)));
    let router_handle = tokio::spawn(router::run_router(rx, cache.clone(), notify_tx));
    let strategy_handle = tokio::spawn(strategy::run_strategy_engine(
        notify_rx, cache.clone(), strategies, signal_tx,
    ));
    let signal_handle = tokio::spawn(strategy::run_signal_logger(signal_rx));

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
        res = strategy_handle => {
            match res {
                Ok(()) => warn!("strategy engine exited"),
                Err(err) => warn!(error = %err, "strategy engine task panicked"),
            }
        }
        res = signal_handle => {
            match res {
                Ok(()) => warn!("signal logger exited"),
                Err(err) => warn!(error = %err, "signal logger task panicked"),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl-C, shutting down");
        }
    }

    Ok(())
}
