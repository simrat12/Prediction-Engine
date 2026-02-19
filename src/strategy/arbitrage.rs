use crate::market_data::types::Side;
use crate::state::market::MarketState;
use crate::state::market_cache::MarketKey;
use super::traits::{Strategy, TradeSignal};
use std::time::Instant;

/// Detects intra-market arbitrage on binary prediction markets.
///
/// When best_bid + best_ask > 1.0, there is an overpricing:
/// selling both outcome tokens yields more than 1.0 combined.
/// The `min_edge` threshold filters noise from rounding and fees.
pub struct ArbitrageStrategy {
    min_edge: f64,
}

impl ArbitrageStrategy {
    pub fn new(min_edge: f64) -> Self {
        Self { min_edge }
    }
}

impl Strategy for ArbitrageStrategy {
    fn name(&self) -> &'static str {
        "arbitrage"
    }

    fn evaluate(&self, key: &MarketKey, state: &MarketState) -> Option<TradeSignal> {
        let bid = state.best_bid?;
        let ask = state.best_ask?;

        let edge = bid + ask - 1.0;

        if edge >= self.min_edge {
            Some(TradeSignal {
                strategy_name: self.name(),
                venue: key.0.clone(),
                market_id: key.1.clone(),
                side: Side::Sell,
                edge,
                observed_bid: Some(bid),
                observed_ask: Some(ask),
                generated_at: Instant::now(),
            })
        } else {
            None
        }
    }
}
