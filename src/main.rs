#![allow(warnings)]

mod config;

pub use tracing_subscriber::filter::EnvFilter;
pub use anyhow::Result;
pub use tracing::{info, warn};
use tokio::sync::mpsc;
use std::sync::Arc;
use prediction_engine::market_data::router;
use prediction_engine::market_data::market_worker::Notification;
use prediction_engine::state::market_cache::MarketCache;
use prediction_engine::market_data::adapters::polymarket;
use prediction_engine::strategy;
use prediction_engine::strategy::traits::TradeSignal;
use prediction_engine::strategy::arbitrage::ArbitrageStrategy;
use prediction_engine::execution;
use prediction_engine::execution::paper::PaperExecutor;

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

    let cache = MarketCache::new();

    // Initialize adapter — fetches markets and returns metadata + spawned handle
    let pm = polymarket::init_polymarket_adapter(tx).await?;

    let market_map = Arc::new(pm.market_map);
    let token_to_market = pm.token_to_market;

    info!(
        markets = market_map.len(),
        tokens = token_to_market.len(),
        "market metadata loaded"
    );

    // MarketWorker → StrategyEngine notification channel
    let (notify_tx, notify_rx) = mpsc::channel::<Notification>(NOTIFY_CHANNEL_BUFFER);

    // StrategyEngine → ExecutionBridge signal channel
    let (signal_tx, signal_rx) = mpsc::channel::<TradeSignal>(SIGNAL_CHANNEL_BUFFER);

    let strategies: Vec<Box<dyn strategy::traits::Strategy>> = vec![
        Box::new(ArbitrageStrategy::new(0.01, 5.0)),
    ];

    let router_handle = tokio::spawn(router::run_router(rx, cache.clone(), notify_tx));
    let strategy_handle = tokio::spawn(strategy::run_strategy_engine(
        notify_rx, cache.clone(), strategies, signal_tx,
        Arc::clone(&market_map), Arc::clone(&token_to_market),
    ));
    let exec_handle = tokio::spawn(execution::run_execution_bridge(
        signal_rx,
        Box::new(PaperExecutor::new()),
        "paper",
    ));

    tokio::select! {
        res = pm.handle => {
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
        res = exec_handle => {
            match res {
                Ok(()) => warn!("execution bridge exited"),
                Err(err) => warn!(error = %err, "execution bridge task panicked"),
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("received Ctrl-C, shutting down");
        }
    }

    Ok(())
}
