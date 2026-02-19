#![allow(warnings)]

use tokio::sync::mpsc;
use tracing::{debug, warn};
use crate::market_data::types::MarketEvent;
use crate::state::market::MarketState;
use crate::state::market_cache::{MarketCache, MarketKey, insert};

pub async fn run_market_worker(
    mut rx: mpsc::Receiver<MarketEvent>,
    handle: MarketCache,
    notify_tx: mpsc::Sender<MarketKey>,
) -> anyhow::Result<()> {
    while let Some(event) = rx.recv().await {
        let key = MarketKey(event.venue.clone(), event.token_id.clone());

        let state = MarketState {
            best_bid: event.best_bid,
            best_ask: event.best_ask,
            volume24h: event.volume24h,
        };

        debug!(
            market_id = %event.market_id,
            ?state,
            "updating cache"
        );

        insert(&handle, key.clone(), state);

        // Notify strategy engine — non-blocking so the data path
        // never stalls on a slow strategy consumer.
        let _ = notify_tx.try_send(key);
    }

    Ok(())
}
