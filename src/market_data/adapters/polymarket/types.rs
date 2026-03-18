use std::collections::HashMap;
use polymarket_rs::types::GammaMarket;

// ── Public types used by the strategy engine and main ────────────────────────

/// Metadata for a binary (YES/NO) prediction market on Polymarket.
#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub market_id: String,
    pub question: String,
    /// Token ID for the YES outcome.
    pub yes_token_id: String,
    /// Token ID for the NO outcome.
    pub no_token_id: String,
    pub neg_risk: bool,
}

/// market_id → MarketInfo lookup table.
pub type MarketMap = HashMap<String, MarketInfo>;

/// token_id → market_id reverse lookup.
/// Used by the WS handler to map incoming token events back to their market.
pub type TokenToMarket = HashMap<String, String>;

// ── Internal types used only during startup ───────────────────────────────────

/// A market that has passed all eligibility filters during startup.
/// Exists only during initialisation — not exposed outside the adapter.
pub(super) struct EligibleMarket {
    pub market_id: String,
    pub question: String,
    /// Always exactly 2 entries: [yes_token_id, no_token_id].
    pub token_ids: Vec<String>,
    /// Lifetime cumulative volume (USD).
    pub volume: f64,
    pub last_trade_price: Option<f64>,
    pub liquidity: Option<f64>,
    pub neg_risk: bool,
}

// ── Market eligibility filter ─────────────────────────────────────────────────

/// Parse a raw Gamma API market and decide whether it's eligible for trading.
///
/// A market is eligible when it:
/// - is active, not closed, not archived
/// - is binary (exactly 2 CLOB token IDs)
/// - has non-zero outcome prices
/// - has ≥ $100K 24-hour volume
/// - has ≥ $10K liquidity
///
/// Returns `None` if any condition fails.
pub(super) fn try_parse_eligible(m: &GammaMarket) -> Option<EligibleMarket> {
    if !m.active || m.closed || m.archived {
        return None;
    }

    let raw_ids = m.clob_token_ids.as_deref()?;
    let raw_prices = m.outcome_prices.as_deref()?;

    // outcome_prices is a JSON array of decimal strings e.g. "[\"0.72\", \"0.28\"]"
    let prices: Vec<f64> = serde_json::from_str::<Vec<String>>(raw_prices)
        .ok()
        .map(|v| v.into_iter().filter_map(|p| p.parse::<f64>().ok()).collect())
        .unwrap_or_default();

    // Require exactly 2 outcome prices that together cover the probability space.
    // Multi-outcome markets can sneak through `ids.len() == 2` if the Gamma API
    // happens to return only 2 tokens (e.g. two ~19% legs of a 5-way market).
    // A genuine binary YES/NO pair always sums to ~1.0; anything well below that
    // (e.g. 0.19 + 0.19 = 0.38) is two independent outcomes, not complements.
    if prices.len() != 2 {
        return None;
    }
    let price_sum = prices[0] + prices[1];
    if !(0.85..=1.15).contains(&price_sum) {
        return None; // not complementary YES/NO outcomes
    }

    // clob_token_ids is also a JSON array string
    let ids: Vec<String> = serde_json::from_str(raw_ids).ok()?;
    if ids.len() != 2 {
        return None; // not a binary market
    }

    if m.volume24hr.unwrap_or(0.0) < 100_000.0 {
        return None;
    }
    if m.liquidity_num.unwrap_or(0.0) < 10_000.0 {
        return None;
    }

    let volume = m
        .volume
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0);

    Some(EligibleMarket {
        market_id: m.id.clone(),
        question: m.question.clone(),
        token_ids: ids,
        volume,
        last_trade_price: m.last_trade_price,
        liquidity: m.liquidity.as_ref().and_then(|l| l.parse::<f64>().ok()),
        neg_risk: false,
    })
}
