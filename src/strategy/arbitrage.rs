use crate::market_data::types::Side;
use crate::state::market_cache::MarketKey;
use super::traits::{Strategy, TradeSignal, SignalLeg, EvalContext};
use std::time::Instant;

/// Detects cross-outcome arbitrage on binary prediction markets.
///
/// Sell arb: YES_bid + NO_bid > 1.0 — sell both outcomes for guaranteed profit.
/// Buy arb:  YES_ask + NO_ask < 1.0 — buy both outcomes for guaranteed profit.
pub struct ArbitrageStrategy {
    min_edge: f64,
    default_size: f64,
}

impl ArbitrageStrategy {
    pub fn new(min_edge: f64, default_size: f64) -> Self {
        Self { min_edge, default_size }
    }
}

impl Strategy for ArbitrageStrategy {
    fn name(&self) -> &'static str {
        "arbitrage"
    }

    fn evaluate(&self, ctx: &EvalContext) -> Option<TradeSignal> {
        let token_id = &ctx.updated_key.1;
        let venue = &ctx.updated_key.0;

        // Look up which market this token belongs to
        let market_id = ctx.token_to_market.get(token_id)?;
        let info = ctx.market_map.get(market_id)?;

        // Read both YES and NO token states from cache
        let yes_key = MarketKey(venue.clone(), info.yes_token_id.clone());
        let no_key = MarketKey(venue.clone(), info.no_token_id.clone());

        let yes_state = ctx.cache.get_market_state(&yes_key)?;
        let no_state = ctx.cache.get_market_state(&no_key)?;

        let yes_bid = yes_state.best_bid?;
        let no_bid = no_state.best_bid?;
        let yes_ask = yes_state.best_ask?;
        let no_ask = no_state.best_ask?;

        // Sell arb: sell YES + sell NO when combined bids exceed 1.0
        let sell_edge = yes_bid + no_bid - 1.0;
        if sell_edge >= self.min_edge {
            return Some(TradeSignal {
                strategy_name: self.name(),
                venue: venue.clone(),
                market_id: market_id.clone(),
                legs: vec![
                    SignalLeg {
                        token_id: info.yes_token_id.clone(),
                        side: Side::Sell,
                        price: yes_bid,
                        size: self.default_size,
                    },
                    SignalLeg {
                        token_id: info.no_token_id.clone(),
                        side: Side::Sell,
                        price: no_bid,
                        size: self.default_size,
                    },
                ],
                edge: sell_edge,
                generated_at: Instant::now(),
            });
        }

        // Buy arb: buy YES + buy NO when combined asks are below 1.0
        let buy_edge = 1.0 - (yes_ask + no_ask);
        if buy_edge >= self.min_edge {
            return Some(TradeSignal {
                strategy_name: self.name(),
                venue: venue.clone(),
                market_id: market_id.clone(),
                legs: vec![
                    SignalLeg {
                        token_id: info.yes_token_id.clone(),
                        side: Side::Buy,
                        price: yes_ask,
                        size: self.default_size,
                    },
                    SignalLeg {
                        token_id: info.no_token_id.clone(),
                        side: Side::Buy,
                        price: no_ask,
                        size: self.default_size,
                    },
                ],
                edge: buy_edge,
                generated_at: Instant::now(),
            });
        }

        None
    }
}
