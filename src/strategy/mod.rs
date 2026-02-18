pub mod traits;
pub mod arbitrage;
pub mod simple;

use tokio::sync::mpsc;
use tracing::{info, warn, debug};
use crate::state::market_cache::{MarketCache, MarketKey};
use traits::{Strategy, TradeSignal};

/// Receives MarketKey notifications on every cache update,
/// reads the latest state, and runs all registered strategies.
/// Signals are forwarded to signal_tx for downstream consumption.
pub async fn run_strategy_engine(
    mut notify_rx: mpsc::Receiver<MarketKey>,
    cache: MarketCache,
    strategies: Vec<Box<dyn Strategy>>,
    signal_tx: mpsc::Sender<TradeSignal>,
) {
    info!(
        strategy_count = strategies.len(),
        "strategy engine started"
    );

    while let Some(key) = notify_rx.recv().await {
        let Some(state) = cache.get_market_state(&key) else {
            debug!(?key, "cache miss for notified key");
            continue;
        };

        for strategy in &strategies {
            if let Some(signal) = strategy.evaluate(&key, &state) {
                info!(
                    strategy = signal.strategy_name,
                    market_id = %signal.market_id,
                    edge = %signal.edge,
                    "trade signal generated"
                );

                if signal_tx.send(signal).await.is_err() {
                    warn!("signal channel closed, stopping strategy engine");
                    return;
                }
            }
        }
    }

    info!("notification channel closed, strategy engine shutting down");
}

/// Logs all trade signals with end-to-end latency.
/// Placeholder for the execution bridge.
pub async fn run_signal_logger(mut signal_rx: mpsc::Receiver<TradeSignal>) {
    while let Some(signal) = signal_rx.recv().await {
        let latency = signal.generated_at.elapsed();
        info!(
            strategy = signal.strategy_name,
            venue = ?signal.venue,
            market_id = %signal.market_id,
            side = ?signal.side,
            edge = %signal.edge,
            bid = ?signal.observed_bid,
            ask = ?signal.observed_ask,
            latency_us = latency.as_micros(),
            "TRADE SIGNAL"
        );
    }
}
