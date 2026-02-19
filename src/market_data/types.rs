#![allow(dead_code)]

use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Venue {
    Polymarket,
    Kalshi
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarketEventKind {
    Trade{price: f64, size: f64, side: Side},
    TopOfBook{bid_price: f64, bid_size: f64, ask_price: f64, ask_size: f64},
    Heartbeat,
    PriceChange
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarketEvent {
    pub venue: Venue,
    pub kind: MarketEventKind,
    pub market_id: String,
    pub token_id: String,
    pub ts_exchange_ms: Option<SystemTime>,
    pub ts_receive_ms: Option<SystemTime>,
    pub volume24h: Option<f64>,
    pub last_trade_price: Option<f64>,
    pub liquidity: Option<f64>,
    pub best_bid: Option<f64>,
    pub best_ask: Option<f64>,
}