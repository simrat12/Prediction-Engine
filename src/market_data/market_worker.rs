use tokio::sync::mpsc;
use std::time::{SystemTime, Duration};
use crate::market_data::types::MarketEvent;

pub async fn run_market_worker(mut rx: mpsc::Receiver<MarketEvent>) -> anyhow::Result<()> {

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
    }

    ;
    Ok(())
}