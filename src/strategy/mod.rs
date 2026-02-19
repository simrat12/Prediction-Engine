pub mod traits;
pub mod arbitrage;
pub mod simple;

use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn, debug};
use crate::market_data::adapters::polymarket::{MarketMap, TokenToMarket};
use crate::state::market_cache::{MarketCache, MarketKey};
use traits::{Strategy, TradeSignal, EvalContext};

/// Receives MarketKey notifications on every cache update,
/// reads the latest state, and runs all registered strategies.
/// Signals are forwarded to signal_tx for downstream consumption.
pub async fn run_strategy_engine(
    mut notify_rx: mpsc::Receiver<MarketKey>,
    cache: MarketCache,
    strategies: Vec<Box<dyn Strategy>>,
    signal_tx: mpsc::Sender<TradeSignal>,
    market_map: Arc<MarketMap>,
    token_to_market: Arc<TokenToMarket>,
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

        let ctx = EvalContext {
            updated_key: &key,
            updated_state: &state,
            cache: &cache,
            market_map: &market_map,
            token_to_market: &token_to_market,
        };

        for strategy in &strategies {
            if let Some(signal) = strategy.evaluate(&ctx) {
                info!(
                    strategy = signal.strategy_name,
                    market_id = %signal.market_id,
                    edge = %signal.edge,
                    legs = signal.legs.len(),
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
