use tokio::sync::mpsc;
use std::time::{SystemTime, Duration};
use crate::market_data::types::MarketEvent;
use crate::state::market::MarketState;
use std::collections::HashMap;
use crate::market_data::types::Venue;

pub async fn run_market_worker(mut rx: mpsc::Receiver<MarketEvent>) -> anyhow::Result<()> {
    let mut market_state: HashMap<Venue, MarketState> = HashMap::new();

    while let Some(mut event) = rx.recv().await {
        match event.ts_exchange_ms {
            Some(ts) => {
                let time_elapsed = SystemTime::now().duration_since(ts).unwrap_or_else(|_| Duration::from_secs(0));
                event.ts_receive_ms = Some(time_elapsed);
                market_state.entry(event.venue.clone()).or_insert(MarketState {
                    last_event: None,
                    best_bid: None,
                    best_ask: None,
                }).last_event = Some(event.clone());
            },
            None => {
                println!("No exchange timestamp");
            }
        }
    }

    ;
    Ok(())
}