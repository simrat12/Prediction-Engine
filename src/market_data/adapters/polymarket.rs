use tokio::sync::mpsc;

use crate::market_data::{adapters::polymarket, types::{MarketEvent, MarketEventKind, Venue}};
use std::time::SystemTime;
use polymarket_rs;


pub async fn run_polymarket_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    let polymarket_client = polymarket_rs::ClobClient ::new("https://clob.polymarket.com");

    let markets = polymarket_client.get_markets(None).await?;
    
    tokio::spawn(async move {

        for i in 0..1 {

            let event = MarketEvent {
                venue: Venue::Polymarket,
                kind: MarketEventKind::Heartbeat,
                market_id: format!("market_{}", i),
                ts_exchange_ms: Some(SystemTime::now()),
                ts_receive_ms: None,   
            };

            println!("markets are here:{:?}", &markets.data[0..10]);


            if tx.send(event).await.is_err() {
                println!("channel closed");
            } else {
                println!("Sent event");
            }
        };

    });

    Ok(())
}