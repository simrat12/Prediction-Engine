#![allow(dead_code)]

use std::time::{SystemTime, Duration};

#[derive(Debug, Clone)]
pub enum Venue {
    Polymarket,
    Kalshi
}

#[derive(Debug, Clone)]
pub enum Side {
    Buy,
    Sell
}

#[derive(Debug, Clone)]
pub enum MarketEventKind {
    Trade{price: f64, size: f64, side: Side},
    TopOfBook{bid_price: f64, bid_size: f64, ask_price: f64, ask_size: f64},
    Heartbeat
}

#[derive(Debug, Clone)]
pub struct MarketEvent {
    pub venue: Venue,
    pub kind: MarketEventKind,
    pub market_id: String,
    pub ts_exchange_ms: Option<SystemTime>,
    pub ts_receive_ms: Option<Duration>,
}