use async_trait::async_trait;
use crate::market_data::types::{Venue, Side};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct OrderLeg {
    pub token_id: String,
    pub side: Side,
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone)]
pub struct ExecutionIntent {
    pub venue: Venue,
    pub market_id: String,
    pub strategy_name: &'static str,
    pub legs: Vec<OrderLeg>,
    pub edge: f64,
    pub neg_risk: bool,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub enum LegFillStatus {
    Filled {
        order_id: String,
        avg_price: f64,
        filled_size: f64,
    },
    Rejected {
        reason: String,
    },
    NotAttempted,
}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub market_id: String,
    pub strategy_name: &'static str,
    pub leg_results: Vec<LegFillStatus>,
    pub completed_at: Instant,
}

impl ExecutionReport {
    pub fn fully_filled(&self) -> bool {
        self.leg_results.iter().all(|r| matches!(r, LegFillStatus::Filled { .. }))
    }
}

#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn execute(&self, intent: ExecutionIntent) -> ExecutionReport;
}
