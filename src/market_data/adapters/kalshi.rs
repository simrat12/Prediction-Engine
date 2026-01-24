use tokio::sync::mpsc;
use crate::market_data::{types::{MarketEvent, Venue}};
use std::time::SystemTime;
use kalshi::Kalshi;
use kalshi::TradingEnvironment;

pub async fn run_kalshi_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    let kalshi_instance = Kalshi::new(LiveMarketMode);

    Ok(())
}