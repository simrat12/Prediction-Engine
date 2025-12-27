use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use crate::market_data::types::{MarketEvent, MarketEventKind, Venue};
use std::time::SystemTime;


pub async fn run_router(mut rx: mpsc::Receiver<MarketEvent>) -> anyhow::Result<()> {

    tokio::spawn(async move {

        while let Some(mut event) = rx.recv().await {
            match event.ts_exchange_ms {
                Some(ts) => {
                    let time_elapsed = SystemTime::now().duration_since(ts).unwrap_or_else(|_| Duration::from_secs(0));
                    event.ts_receive_ms = Some(time_elapsed);
                },
                None => {
                    println!("No exchange timestamp");
                }
            }
            println!("Received event: {:?}", event);
        }
    });


    sleep(Duration::from_secs(3)).await;

    Ok(())
}
