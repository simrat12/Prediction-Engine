use crate::market_data::types::{Venue, Side};
use crate::market_data::adapters::polymarket::{MarketMap, TokenToMarket};
use crate::state::market::MarketState;
use crate::state::market_cache::{MarketCache, MarketKey};
use std::time::Instant;

/// A single leg of a multi-leg trade signal.
#[derive(Debug, Clone)]
pub struct SignalLeg {
    pub token_id: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
}

/// Output of a strategy evaluation — a signal, not an order.
/// Supports multi-leg signals for cross-outcome arbitrage.
#[derive(Debug, Clone)]
pub struct TradeSignal {
    pub strategy_name: &'static str,
    pub venue: Venue,
    pub market_id: String,
    pub legs: Vec<SignalLeg>,
    pub edge: f64,
    pub generated_at: Instant,
    /// Monotonic timestamp of when the triggering WS event was received.
    /// Used to measure end-to-end pipeline latency.
    pub ws_received_at: Option<Instant>,
}

/// Context provided to strategies on each cache update.
/// Gives strategies access to the full cache and market metadata
/// so they can read cross-outcome prices.
pub struct EvalContext<'a> {
    pub updated_key: &'a MarketKey,
    pub updated_state: &'a MarketState,
    pub cache: &'a MarketCache,
    pub market_map: &'a MarketMap,
    pub token_to_market: &'a TokenToMarket,
    /// When the triggering WS event was received (monotonic).
    pub ws_received_at: Option<Instant>,
}

/// Trait that all strategies implement.
///
/// Kept synchronous and infallible by design:
/// strategies read from an immutable snapshot, not I/O.
pub trait Strategy: Send + Sync {
    fn name(&self) -> &'static str;

    /// Evaluate against the latest market state for the given key.
    /// Returns Some(TradeSignal) if an opportunity is detected.
    fn evaluate(&self, ctx: &EvalContext) -> Option<TradeSignal>;
}
