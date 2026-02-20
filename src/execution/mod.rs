pub mod traits;
pub mod paper;
pub mod live;

use tokio::sync::mpsc;
use tracing::{info, warn};
use std::time::Instant;

use crate::metrics::prometheus::{
    record_fill, record_rejection, record_signal_to_fill_latency_us, record_e2e_latency_us,
};
use crate::strategy::traits::TradeSignal;
use traits::{ExecutionEngine, ExecutionIntent, OrderLeg, LegFillStatus};

/// Bridges the strategy engine to the execution layer.
/// Converts TradeSignals into ExecutionIntents, dispatches them,
/// and records latency + fill metrics to Prometheus.
pub async fn run_execution_bridge(
    mut signal_rx: mpsc::Receiver<TradeSignal>,
    executor: Box<dyn ExecutionEngine>,
    executor_name: &'static str,
) {
    info!("execution bridge started (executor={})", executor_name);

    while let Some(signal) = signal_rx.recv().await {
        let signal_generated_at = signal.generated_at;
        let ws_received_at = signal.ws_received_at;
        let strategy_name = signal.strategy_name;

        let intent = ExecutionIntent {
            venue: signal.venue,
            market_id: signal.market_id,
            strategy_name,
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
            neg_risk: false,
            created_at: Instant::now(),
        };

        let report = executor.execute(intent).await;

        // ── Record metrics ───────────────────────────────────────────
        let signal_to_fill_us = signal_generated_at.elapsed().as_micros();
        record_signal_to_fill_latency_us(strategy_name, signal_to_fill_us);

        if let Some(ws_at) = ws_received_at {
            let e2e_us = ws_at.elapsed().as_micros();
            record_e2e_latency_us(strategy_name, e2e_us);
        }

        if report.fully_filled() {
            record_fill(strategy_name, executor_name);
            info!(
                strategy = strategy_name,
                market_id = %report.market_id,
                legs = report.leg_results.len(),
                signal_to_fill_us = signal_to_fill_us,
                e2e_us = ws_received_at.map(|t| t.elapsed().as_micros()),
                "execution complete — all legs filled"
            );
        } else {
            let rejected = report.leg_results.iter()
                .filter(|r| matches!(r, LegFillStatus::Rejected { .. }))
                .count();
            if rejected > 0 {
                record_rejection(strategy_name, executor_name);
            }
            warn!(
                strategy = strategy_name,
                market_id = %report.market_id,
                leg_results = ?report.leg_results,
                "execution incomplete — partial or rejected fills"
            );
        }
    }

    info!("signal channel closed, execution bridge shutting down");
}
