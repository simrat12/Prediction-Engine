use tokio::sync::mpsc;

use crate::market_data::{adapters::polymarket, types::{MarketEvent, MarketEventKind, Venue}};
use std::time::SystemTime;
use polymarket_rs::client::GammaClient;
use polymarket_rs::request::GammaMarketParams;
use polymarket_rs;


pub async fn run_polymarket_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    let client = GammaClient::new("https://gamma-api.polymarket.com");

    // Get active markets
    let params = GammaMarketParams::new()
        .with_active(true)
        .with_limit(10);

    let markets = client.get_markets(Some(params)).await?;
    
    tokio::spawn(async move {

        for i in 0..markets.len() {
            
            if markets[i].volume.as_ref().unwrap_or(&"0.0".to_string()).parse::<f64>().unwrap_or(0.0) < 1000000.0 {
                
                let event = MarketEvent {
                    venue: Venue::Polymarket,
                    kind: MarketEventKind::Heartbeat,
                    market_id: markets[i].id.clone(),
                    ts_exchange_ms: Some(SystemTime::now()),
                    ts_receive_ms: None,
                    volume24h: markets[i].volume.as_ref().and_then(|v| v.parse::<f64>().ok()),  
                    last_trade_price: markets[i].last_trade_price,
                    liquidity: markets[i].liquidity.as_ref().and_then(|l| l.parse::<f64>().ok()), 
                    best_bid: markets[i].best_bid,
                    best_ask: markets[i].best_ask,
                };

                if tx.send(event).await.is_err() {
                    println!("channel closed");
                } else {
                    println!("Sent event");
                }
            }

            // println!("markets are here:{:?}", &markets[0..10]);

        };

    });

    Ok(())
}