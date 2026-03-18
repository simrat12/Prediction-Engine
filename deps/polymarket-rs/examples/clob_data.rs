use polymarket_rs::client::ClobClient;
use polymarket_rs::types::TokenId;
use polymarket_rs::{Result, Side};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a CLOB client for market data queries
    let client = ClobClient::new("https://clob.polymarket.com");

    // Check server status
    println!("Checking server status...");
    let status = client.get_ok().await?;
    println!("Server status: {:?}\n", status);

    // Get server time
    let time = client.get_server_time().await?;
    println!("Server time: {:?}\n", time);

    // Example token ID
    let token_id = TokenId::new(
        "7045107161367241233811523851106536676632348173555291268726302515224841822187",
    );

    // Get midpoint price
    println!("Getting midpoint price for token {}...", token_id);
    match client.get_midpoint(&token_id).await {
        Ok(midpoint) => println!("Midpoint price: {}\n", midpoint.mid),
        Err(e) => println!("Error getting midpoint: {}\n", e),
    }

    // Get current price
    println!("Getting current price...");
    match client.get_price(&token_id, Side::Buy).await {
        Ok(price) => println!("Current price: {}\n", price.price),
        Err(e) => println!("Error getting price: {}\n", e),
    }

    // Get spread
    println!("Getting spread...");
    match client.get_spread(&token_id).await {
        Ok(spread) => println!("Spread: {}\n", spread.spread),
        Err(e) => println!("Error getting spread: {}\n", e),
    }

    // Get tick size
    println!("Getting tick size...");
    match client.get_tick_size(&token_id).await {
        Ok(tick_size) => println!("Minimum tick size: {}\n", tick_size.minimum_tick_size),
        Err(e) => println!("Error getting tick size: {}\n", e),
    }

    // Get order book
    println!("Getting order book...");
    match client.get_order_book(&token_id).await {
        Ok(book) => {
            println!("Order book for market: {}", book.market);
            println!("  Bids: {} levels", book.bids.len());
            println!("  Asks: {} levels", book.asks.len());
            if !book.bids.is_empty() {
                println!("  Best bid: {} @ {}", book.bids[0].size, book.bids[0].price);
            }
            if !book.asks.is_empty() {
                println!("  Best ask: {} @ {}", book.asks[0].size, book.asks[0].price);
            }
            println!();
        }
        Err(e) => println!("Error getting order book: {}\n", e),
    }

    // Get sampling markets
    println!("Getting sampling markets...");
    match client.get_sampling_markets(None).await {
        Ok(markets) => {
            println!("Retrieved {} markets", markets.data.len());
            if let Some(market) = markets.data.first() {
                println!("First market condition ID: {}", market.condition_id);
                println!("First market question ID: {}", market.question_id);
                println!("First market question: {}", market.question);
            }
        }
        Err(e) => println!("Error getting markets: {}", e),
    }

    println!("\nExample completed successfully!");
    Ok(())
}
