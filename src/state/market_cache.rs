use crate::state::market::MarketState;
use crate::market_data::types::Venue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MarketCache {
    cache: HashMap<MarketKey, MarketState>,
}

#[derive(Eq, Hash, PartialEq)]
pub struct MarketKey (
    pub Venue,
    pub String, // market_id
);

impl MarketCache {
    pub fn new() -> Self {
        MarketCache {
            cache: HashMap::new(),
        }
    }

    pub fn update_market_state(&mut self, key: MarketKey, state: MarketState) {
        self.cache.insert(key, state);
    }

    pub fn get_market_state(&self, key: &MarketKey) -> Option<&MarketState> {
        self.cache.get(key)
    }
}

type MarketCacheHandle = Arc<RwLock<MarketCache>>;

pub async fn insert(handle: &MarketCacheHandle, key: MarketKey, state: MarketState) {
    let mut cache = handle.write().await;
    cache.update_market_state(key, state);
}