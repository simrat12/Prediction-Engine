use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;

/// Start the Prometheus HTTP exporter on :9000.
/// After this call, any metrics recorded via the `metrics` crate
/// macros (counter!, histogram!) are automatically exported at /metrics.
pub fn init_metrics_server() {
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .expect("failed to start Prometheus metrics server");
}

// ── Adapter metrics ──────────────────────────────────────────────

pub fn record_adapter_event(venue: &str, event_type: &str) {
    counter!("adapter_events_total", "venue" => venue.to_string(), "event_type" => event_type.to_string())
        .increment(1);
}

pub fn record_adapter_latency(venue: &str, event_type: &str, latency_ms: f64) {
    histogram!("adapter_event_latency_ms", "venue" => venue.to_string(), "event_type" => event_type.to_string())
        .record(latency_ms);
}

// ── Strategy metrics ─────────────────────────────────────────────

pub fn record_signal(strategy: &str, venue: &str) {
    counter!("strategy_signals_total", "strategy" => strategy.to_string(), "venue" => venue.to_string())
        .increment(1);
}

pub fn record_signal_edge(strategy: &str, edge: f64) {
    histogram!("strategy_signal_edge", "strategy" => strategy.to_string())
        .record(edge);
}

// ── Execution metrics ────────────────────────────────────────────

pub fn record_fill(strategy: &str, executor: &str) {
    counter!("execution_fills_total", "strategy" => strategy.to_string(), "executor" => executor.to_string())
        .increment(1);
}

pub fn record_rejection(strategy: &str, executor: &str) {
    counter!("execution_rejections_total", "strategy" => strategy.to_string(), "executor" => executor.to_string())
        .increment(1);
}

/// Time from strategy signal generation to execution complete.
pub fn record_signal_to_fill_latency_us(strategy: &str, latency_us: u128) {
    histogram!("execution_signal_to_fill_us", "strategy" => strategy.to_string())
        .record(latency_us as f64);
}

/// Time from WS price receive to execution complete (full pipeline).
pub fn record_e2e_latency_us(strategy: &str, latency_us: u128) {
    histogram!("execution_e2e_latency_us", "strategy" => strategy.to_string())
        .record(latency_us as f64);
}
