use tokio::sync::mpsc;
use crate::market_data::{types::{MarketEvent, Venue}};
use std::time::SystemTime;
use kalshi::Kalshi;
use kalshi::TradingEnvironment;

pub async fn run_kalshi_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    // 1. Create Kalshi WS client (from the fork / kalshi_rust crate)
    let mut kalshi = Kalshi::new_with_api_key(
        TradingEnvironment::LiveMarketMode,
        api_key_id,
        api_private_key,
    );

    let mut ws = kalshi.connect_ws().await?;

    // 2. Subscribe to ticker for 1–2 markets
    ws.subscribe(
        vec![KalshiChannel::Ticker],
        vec!["HIGHNY-23NOV13-T51".to_string()],
    ).await?;

    // 3. Read messages forever
    let mut rx = ws.receiver();

    while let Ok(msg) = rx.recv().await {
        println!("{msg:?}");
    }

}