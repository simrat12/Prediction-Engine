use metrics_exporter_prometheus::PrometheusBuilder;

pub struct Metrics {
    pub adapter_events_total: AdapterEventsTotal,
    pub adapter_event_latency_ms: AdapterEventLatencyMs,
}

pub struct AdapterEventsTotal {
    pub venue: String,
    pub event_type: String,
    pub count: i64,
}

pub struct AdapterEventLatencyMs {
    pub venue: String,
    pub event_type: String,
    pub latency_ms: f64,
}

pub struct StrategySignalsTotal {
    pub strategy_name: String,
    pub signal_type: String,
    pub count: i64,
}

pub fn init_metrics_server() {

    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .expect("Failed to start Prometheus metrics server");
}