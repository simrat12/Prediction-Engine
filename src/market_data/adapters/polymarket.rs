use tokio::sync::mpsc;

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use std::time::SystemTime;


pub async fn run_polymarket_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    
    tokio::spawn(async move {

        for i in 0..10 {

            let event = MarketEvent {
                venue: Venue::Polymarket,
                kind: MarketEventKind::Heartbeat,
                market_id: format!("market_{}", i),
                ts_exchange_ms: Some(SystemTime::now()),
                ts_receive_ms: None,   
            };


            if tx.send(event).await.is_err() {
                println!("channel closed");
            } else {
                println!("Sent event");
            }
        };

    });

    Ok(())
}