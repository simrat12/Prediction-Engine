use crate::market_data::types::{Venue, Side};
use crate::state::market::MarketState;
use crate::state::market_cache::MarketKey;
use std::time::Instant;

/// Output of a strategy evaluation — a signal, not an order.
/// The execution layer decides whether and how to act on it.
#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub strategy_name: &'static str,
    pub venue: Venue,
    pub market_id: String,
    pub side: Side,
    /// Expected edge as a fraction (e.g. 0.02 = 2%).
    pub edge: f64,
    pub observed_bid: Option<f64>,
    pub observed_ask: Option<f64>,
    /// Monotonic timestamp of when the signal was generated.
    pub generated_at: Instant,
}

/// Trait that all strategies implement.
///
/// Kept synchronous and infallible by design:
/// strategies read from an immutable snapshot, not I/O.
/// A strategy that cannot produce a signal returns None.
pub trait Strategy: Send + Sync {
    fn name(&self) -> &'static str;

    /// Evaluate against the latest market state for the given key.
    /// Returns Some(TradeSignal) if an opportunity is detected.
    fn evaluate(&self, key: &MarketKey, state: &MarketState) -> Option<TradeSignal>;
}
