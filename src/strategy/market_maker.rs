use crate::market_data::types::Side;
use super::traits::{Strategy, TradeSignal, SignalLeg, EvalContext};
use std::time::Instant;
use tracing::info;

/// Avellaneda-Stoikov market-making strategy in logit space.
///
/// Uses `polymarket-kernel` to compute model bid/ask quotes via the
/// Avellaneda-Stoikov framework, then compares against live market
/// prices to generate single-leg BUY or SELL signals when there is
/// edge between the model price and the market price.
///
/// # Parameters
///
/// - `gamma`: Risk aversion. Higher = wider model spread (more conservative).
/// - `sigma_b`: Belief volatility in logit space. Higher = wider spread.
/// - `tau`: Time to resolution in year-fractions. Higher = wider spread.
/// - `k`: Liquidity / order-arrival intensity. Higher = tighter spread.
/// - `min_edge`: Minimum edge (in probability units) to emit a signal.
/// - `default_size`: Order size in contracts.
///
/// # Inventory
///
/// Currently uses `q_t = 0` (no inventory tracking). This means the
/// model quotes are symmetric around the market mid. A future version
/// will track inventory from fills and skew quotes accordingly.
pub struct MarketMakerStrategy {
    gamma: f64,
    sigma_b: f64,
    tau: f64,
    k: f64,
    min_edge: f64,
    default_size: f64,
}

impl MarketMakerStrategy {
    pub fn new(
        gamma: f64,
        sigma_b: f64,
        tau: f64,
        k: f64,
        min_edge: f64,
        default_size: f64,
    ) -> Self {
        Self { gamma, sigma_b, tau, k, min_edge, default_size }
    }
}

impl Strategy for MarketMakerStrategy {
    fn name(&self) -> &'static str {
        "market_maker"
    }

    fn evaluate(&self, ctx: &EvalContext) -> Option<TradeSignal> {
        let token_id = &ctx.updated_key.1;
        let venue = &ctx.updated_key.0;

        // ── Step 1: Read market data ───────────────────────────
        let market_bid = ctx.updated_state.best_bid?;
        let market_ask = ctx.updated_state.best_ask?;

        // Sanity: ask must exceed bid
        if market_ask <= market_bid {
            return None;
        }

        // Look up market_id for the TradeSignal
        let market_id = ctx.token_to_market.get(token_id)?;

        // ── Step 2: Compute mid and convert to logit ───────────
        let mid_p = (market_bid + market_ask) / 2.0;

        // Mid must be a valid probability
        if mid_p <= 0.0 || mid_p >= 1.0 {
            return None;
        }

        let x_t = polymarket_kernel::logit(mid_p);

        // ── Step 3: Call kernel (length-1 batch) ───────────────
        // q_t = 0: no inventory → reservation logit = mid logit
        // (symmetric quotes around the market mid).
        let x_t_arr = [x_t];
        let q_t_arr = [0.0_f64];
        let sigma_b_arr = [self.sigma_b];
        let gamma_arr = [self.gamma];
        let tau_arr = [self.tau];
        let k_arr = [self.k];

        let mut kernel_bid_arr = [0.0_f64];
        let mut kernel_ask_arr = [0.0_f64];

        polymarket_kernel::calculate_quotes_logit(
            &x_t_arr,
            &q_t_arr,
            &sigma_b_arr,
            &gamma_arr,
            &tau_arr,
            &k_arr,
            &mut kernel_bid_arr,
            &mut kernel_ask_arr,
        );

        let kernel_bid = kernel_bid_arr[0];
        let kernel_ask = kernel_ask_arr[0];

        // ── Step 4: Compare kernel vs market ───────────────────
        //
        // BUY  when market_ask < kernel_bid
        //   → market is offering cheaper than our model would bid
        //   → edge = kernel_bid - market_ask
        //
        // SELL when market_bid > kernel_ask
        //   → market is bidding higher than our model would ask
        //   → edge = market_bid - kernel_ask

        let buy_edge = kernel_bid - market_ask;
        let sell_edge = market_bid - kernel_ask;

        // ── Step 5: Diagnostic logging ─────────────────────────
        if buy_edge > 0.0 || sell_edge > 0.0 {
            info!(
                token_id = %token_id,
                market_id = %market_id,
                mid_p,
                market_bid,
                market_ask,
                kernel_bid,
                kernel_ask,
                buy_edge,
                sell_edge,
                gamma = self.gamma,
                sigma_b = self.sigma_b,
                tau = self.tau,
                k = self.k,
                "market_maker edge detected"
            );
        }

        // ── Step 6: Emit signal if edge passes threshold ───────
        if buy_edge >= self.min_edge {
            return Some(TradeSignal {
                strategy_name: self.name(),
                venue: venue.clone(),
                market_id: market_id.clone(),
                legs: vec![SignalLeg {
                    token_id: token_id.clone(),
                    side: Side::Buy,
                    price: market_ask,
                    size: self.default_size,
                }],
                edge: buy_edge,
                generated_at: Instant::now(),
                ws_received_at: ctx.ws_received_at,
            });
        }

        if sell_edge >= self.min_edge {
            return Some(TradeSignal {
                strategy_name: self.name(),
                venue: venue.clone(),
                market_id: market_id.clone(),
                legs: vec![SignalLeg {
                    token_id: token_id.clone(),
                    side: Side::Sell,
                    price: market_bid,
                    size: self.default_size,
                }],
                edge: sell_edge,
                generated_at: Instant::now(),
                ws_received_at: ctx.ws_received_at,
            });
        }

        None
    }
}
