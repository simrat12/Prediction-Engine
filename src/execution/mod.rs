pub mod traits;
pub mod paper;
pub mod live;

use tokio::sync::mpsc;
use tracing::{info, warn};
use std::time::Instant;

use crate::strategy::traits::TradeSignal;
use traits::{ExecutionEngine, ExecutionIntent, OrderLeg};

/// Bridges the strategy engine to the execution layer.
/// Converts TradeSignals into ExecutionIntents and dispatches them.
pub async fn run_execution_bridge(
    mut signal_rx: mpsc::Receiver<TradeSignal>,
    executor: Box<dyn ExecutionEngine>,
) {
    info!("execution bridge started");

    while let Some(signal) = signal_rx.recv().await {
        let signal_latency = signal.generated_at.elapsed();

        let intent = ExecutionIntent {
            venue: signal.venue,
            market_id: signal.market_id,
            strategy_name: signal.strategy_name,
            legs: signal
                .legs
                .into_iter()
                .map(|leg| OrderLeg {
                    token_id: leg.token_id,
                    side: leg.side,
                    price: leg.price,
                    size: leg.size,
                })
                .collect(),
            edge: signal.edge,
            neg_risk: false, // TODO: pass through from MarketInfo when available
            created_at: Instant::now(),
        };

        let report = executor.execute(intent).await;

        if report.fully_filled() {
            info!(
                strategy = report.strategy_name,
                market_id = %report.market_id,
                legs = report.leg_results.len(),
                signal_latency_us = signal_latency.as_micros(),
                "execution complete — all legs filled"
            );
        } else {
            warn!(
                strategy = report.strategy_name,
                market_id = %report.market_id,
                leg_results = ?report.leg_results,
                "execution incomplete — partial or rejected fills"
            );
        }
    }

    info!("signal channel closed, execution bridge shutting down");
}
