use crate::market_data::types::MarketEvent;

pub struct MarketState {
    pub last_event: Option<MarketEvent>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
}