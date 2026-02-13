use std::collections::HashMap;
use metrics_exporter_prometheus::PrometheusBuilder;

pub struct Metrics {
    pub adapter_events_total: AdapterEventsTotal,
    pub adapter_event_latency_ms: AdapterEventLatencyMs,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            adapter_events_total: AdapterEventsTotal::new(),
            adapter_event_latency_ms: AdapterEventLatencyMs::new(),
        }
    }

    pub fn increment_counter(&mut self, venue: &str, event_type: &str) {
        self.adapter_events_total.increment(venue, event_type);
    }

    pub fn observe_latency(&mut self, venue: &str, event_type: &str, latency_ms: f64) {
        self.adapter_event_latency_ms.observe(venue, event_type, latency_ms);
    }
}

pub struct AdapterEventsTotal {
    pub counts: HashMap<(String, String), i64>,
}

impl AdapterEventsTotal {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    pub fn increment(&mut self, venue: &str, event_type: &str) {
        let key = (venue.to_string(), event_type.to_string());
        *self.counts.entry(key).or_insert(0) += 1;
    }

    pub fn get(&self, venue: &str, event_type: &str) -> i64 {
        let key = (venue.to_string(), event_type.to_string());
        self.counts.get(&key).copied().unwrap_or(0)
    }
}

pub struct LatencyStats {
    pub count: u64,
    pub total_ms: f64,
}

pub struct AdapterEventLatencyMs {
    pub stats: HashMap<(String, String), LatencyStats>,
}

impl AdapterEventLatencyMs {
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
        }
    }

    pub fn observe(&mut self, venue: &str, event_type: &str, latency_ms: f64) {
        let key = (venue.to_string(), event_type.to_string());
        let entry = self.stats.entry(key).or_insert(LatencyStats { count: 0, total_ms: 0.0 });
        entry.count += 1;
        entry.total_ms += latency_ms;
    }

    pub fn avg_ms(&self, venue: &str, event_type: &str) -> Option<f64> {
        let key = (venue.to_string(), event_type.to_string());
        self.stats.get(&key).map(|s| s.total_ms / s.count as f64)
    }
}

pub struct StrategySignalsTotal {
    pub counts: std::collections::HashMap<(String, String), i64>,
}

impl StrategySignalsTotal {
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    pub fn increment(&mut self, strategy_name: &str, signal_type: &str) {
        let key = (strategy_name.to_string(), signal_type.to_string());
        *self.counts.entry(key).or_insert(0) += 1;
    }

    pub fn get(&self, strategy_name: &str, signal_type: &str) -> i64 {
        let key = (strategy_name.to_string(), signal_type.to_string());
        self.counts.get(&key).copied().unwrap_or(0)
    }
}

pub fn init_metrics_server() {

    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9000))
        .install()
        .expect("Failed to start Prometheus metrics server");
}