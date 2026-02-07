use crate::state::market::MarketState;
use crate::market_data::types::Venue;
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub struct MarketKey(
    pub Venue,
    pub String, // market_id
);

/// Thread-safe market cache backed by DashMap.
/// Eliminates write-lock contention: concurrent writers on different keys
/// never block each other, and readers are never blocked by writers.
#[derive(Clone, Debug)]
pub struct MarketCache {
    cache: Arc<DashMap<MarketKey, MarketState>>,
}

/// Shared handle to the cache — just a cheap Arc clone.
pub type MarketCacheHandle = MarketCache;

impl MarketCache {
    pub fn new() -> Self {
        MarketCache {
            cache: Arc::new(DashMap::new()),
        }
    }

    pub fn update_market_state(&self, key: MarketKey, state: MarketState) {
        self.cache.insert(key, state);
    }

    pub fn get_market_state(&self, key: &MarketKey) -> Option<MarketState> {
        self.cache.get(key).map(|entry| entry.value().clone())
    }

    pub fn get_markets_by_venue(&self, venue: &Venue) -> Vec<(MarketKey, MarketState)> {
        self.cache
            .iter()
            .filter(|entry| &entry.key().0 == venue)
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }
}

/// Insert or update a market state in the cache.
/// No async lock required — DashMap handles synchronization internally.
pub fn insert(handle: &MarketCacheHandle, key: MarketKey, state: MarketState) {
    handle.update_market_state(key, state);
}
