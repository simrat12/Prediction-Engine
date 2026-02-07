/// Lightweight snapshot of the latest market data.
/// Stores only the pricing/volume fields — no redundant full-event clone.
#[derive(Clone, Debug)]
pub struct MarketState {
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub volume24h: Option<f64>,
}
