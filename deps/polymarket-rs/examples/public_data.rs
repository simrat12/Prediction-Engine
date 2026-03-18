use polymarket_rs::client::DataClient;
use polymarket_rs::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a DataClient for accessing position and portfolio data
    let client = DataClient::new("https://data-api.polymarket.com");

    // Example wallet address
    let user_address = "0xe0368af7f5777989b927b7ad0d420562fee8616c";

    // Get all positions for a user
    println!("Fetching positions for user: {}...\n", user_address);
    match client.get_positions(user_address).await {
        Ok(positions) => {
            println!("Found {} positions:", positions.len());

            for (i, position) in positions.iter().enumerate() {
                println!("\nPosition {}:", i + 1);
                println!("  Market: {}", position.title);
                println!("  Outcome: {}", position.outcome);
                println!("  Size: {}", position.size);
                println!("  Average Price: {}", position.avg_price);
                println!("  Current Price: {}", position.cur_price);
                println!("  Current Value: ${}", position.current_value);
                println!("  Cash P&L: ${}", position.cash_pnl);
                println!("  Percent P&L: {}%", position.percent_pnl);
                println!("  Realized P&L: ${}", position.realized_pnl);
                println!("  Redeemable: {}", position.redeemable);
                println!("  End Date: {}", position.end_date);

                // Only show first 5 positions to keep output manageable
                if i >= 4 {
                    println!("\n... and {} more positions", positions.len() - 5);
                    break;
                }
            }
        }
        Err(e) => println!("Error fetching positions: {}", e),
    }

    // Get the total value of positions for a user
    println!("Fetching position value for user: {}...\n", user_address);
    match client.get_positions_value(user_address).await {
        Ok(values) => {
            println!("Position values:");
            for value in values {
                println!("  User: {}", value.user);
                println!("  Total Value: ${}", value.value);
            }
        }
        Err(e) => println!("Error fetching position value: {}", e),
    }

    println!("\nExample completed successfully!");
    Ok(())
}
