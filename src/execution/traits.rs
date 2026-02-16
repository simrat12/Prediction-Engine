use crate::market_data::types::{Venue, Side};


pub struct ExecutionIntent {
    pub Venue: Venue,
    pub MarketId: String,
    pub Side: Side,
    pub Size: u64,
    pub limit_price: Option<u64>
}

pub struct ExecutionReport {
    pub order_id: u64,
    pub Venue: Venue,
    pub filled_size: Option<u64>,
    pub avg_price: Option<u64>,
    pub fees: Option<u64>,
    pub status: Status,
    pub timestamp: Duration
}

pub trait ExecutionEngine {
    pub fn submit(intent: ExecutionIntent) -> ExecutionReport;
    pub fn cancel(order_id: u64) -> ExecutionReport;
    pub fn status(order_id: u64) -> ExecutionReport;
}