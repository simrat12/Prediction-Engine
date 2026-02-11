pub mod prometheus;

pub fn init_metrics() {
	prometheus::init_metrics_server();
}