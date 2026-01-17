# Prediction Engine (Rust)

A modular trading and research system for prediction markets
(Polymarket, Kalshi).

## Goals
- Learn async Rust and system design
- Ingest prediction market data
- Maintain internal market & position state
- Implement paper trading strategies
- Gradually evolve toward production-grade execution

## Status
🚧 Early development – educational project.

## Running
```bash
cargo run


## Data Flow Architecture

```text
┌─────────────────────────────┐
│ adapters/polymarket.rs      │
│ - fetch                     │
│ - filter vol >= 1m          │
│ - build MarketEvent         │
└──────────────┬──────────────┘
               │ tx_events.send(event)
               ▼
     ┌──────────────────────┐
     │ main.rs               │
     │ - create channel      │
     │ - start router        │
     └──────────┬───────────┘
                ▼
     ┌──────────────────────┐
     │ router.rs             │
     │ - lanes per venue     │
     │ - spawn worker        │
     └──────────┬───────────┘
                │ lanes[venue].send(event)
                ▼
┌──────────────────────────────┐
│ market_worker.rs              │
│ - recv per-venue lane         │
│ - merge event → MarketState  │
│ - write to cache              │
└───────────┬─────────────────┘
            ▼
┌──────────────────────────────┐
│ state/market_cache.rs         │
│ - Arc<RwLock<MarketCache>>    │
│ - (venue, market_id) → state │
└───────────┬─────────────────┘
            ▼
┌──────────────────────────────┐
│ strategy / execution          │
│ - read cache                  │
│ - decide / trade              │
└──────────────────────────────┘
