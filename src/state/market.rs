/// Lightweight snapshot of the latest market data.
/// Stores only the pricing/volume fields — no redundant full-event clone.
#[derive(Clone, Debug)]
pub struct MarketState {
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub volume24h: Option<f64>,
}

impl MarketState {
    /// Merge a partial update into this state.
    /// Only overwrites fields that are `Some` in `update`; leaves others unchanged.
    pub fn merge(&mut self, update: &MarketState) {
        if update.best_bid.is_some() {
            self.best_bid = update.best_bid;
        }
        if update.best_ask.is_some() {
            self.best_ask = update.best_ask;
        }
        if update.volume24h.is_some() {
            self.volume24h = update.volume24h;
        }
    }
}
