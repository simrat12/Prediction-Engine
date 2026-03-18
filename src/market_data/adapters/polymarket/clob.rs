use polymarket_rs::{ClobClient, Side};
use polymarket_rs::types::TokenId;
use rust_decimal::Decimal;
use tracing::warn;

/// Fetch bid and ask prices for a single token from the CLOB REST API.
///
/// ## Polymarket API naming (counterintuitive)
///
/// The `/price` endpoint uses the caller's perspective, not the book's:
///
/// | Request side | Meaning                    | Maps to   |
/// |--------------|----------------------------|-----------|
/// | `side=BUY`   | Price *to buy* this token  | best ask  |
/// | `side=SELL`  | Price *to sell* this token | best bid  |
///
/// So `buy_price` = what you pay to acquire = **best ask**,
/// and `sell_price` = what you receive when selling = **best bid**.
///
/// The caller is responsible for mapping correctly:
/// ```
/// let (buy_price, sell_price) = fetch_prices(...).await?;
/// let best_ask = buy_price;
/// let best_bid = sell_price;
/// ```
///
/// Returns `None` if either leg fails (logged as a warning).
pub(super) async fn fetch_prices(
    clob_client: &ClobClient,
    token_id: &str,
    market_id: &str,
) -> Option<(Decimal, Decimal)> {
    let tid = TokenId::from(token_id.to_owned());

    // Issue both requests in parallel — no need to wait for one before the other.
    let (buy_res, sell_res) = tokio::join!(
        clob_client.get_price(&tid, Side::Buy),
        clob_client.get_price(&tid, Side::Sell),
    );

    let buy_price = match buy_res {
        Ok(p) => p.price,
        Err(e) => {
            warn!(market_id, error = %e, "CLOB buy-price fetch failed");
            return None;
        }
    };

    let sell_price = match sell_res {
        Ok(p) => p.price,
        Err(e) => {
            warn!(market_id, error = %e, "CLOB sell-price fetch failed");
            return None;
        }
    };

    Some((buy_price, sell_price))
}
