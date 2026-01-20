use crate::market_data::types::MarketEvent;

#[derive(Clone, Debug)]
pub struct MarketState {
    pub last_event: Option<MarketEvent>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
    pub volume24h: Option<f64>,
}