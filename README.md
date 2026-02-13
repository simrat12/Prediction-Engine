# Prediction Engine

A modular trading system for prediction markets (Polymarket, Kalshi (ToDo)) built in async Rust.

## Architecture

```
main.rs
  ├─ Initializes tracing, Prometheus metrics server (:9000)
  ├─ Creates mpsc channel + DashMap-backed MarketCache
  ├─ Spawns adapter tasks (Polymarket, Kalshi)
  └─ Spawns event router

adapters/polymarket.rs
  ├─ Fetches eligible markets from Gamma API
  ├─ Filters by volume, liquidity, and CLOB tradability
  ├─ Fetches initial prices via buffered concurrent HTTP (10x)
  ├─ Subscribes to CLOB WebSocket for real-time price changes
  ├─ Reconnects with exponential backoff (up to 10 attempts)
  └─ Emits MarketEvent → mpsc channel

router.rs
  ├─ Receives MarketEvents from all adapters
  ├─ Lazily spawns per-venue market_worker tasks
  └─ Forwards events to the appropriate venue lane

market_worker.rs
  ├─ Consumes venue-specific events
  ├─ Merges partial updates into MarketState
  └─ Writes to MarketCache (lock-free via DashMap)

state/market_cache.rs
  ├─ DashMap<(Venue, market_id), MarketState>
  ├─ Concurrent reads/writes without lock contention
  └─ Partial merge support (only overwrites Some fields)

metrics/prometheus.rs
  ├─ Adapter event counters keyed by (venue, event_type)
  ├─ Latency tracking with running stats per (venue, event_type)
  ├─ Strategy signal counters keyed by (strategy, signal_type)
  └─ Prometheus exporter on :9000/metrics

strategy/
  ├─ traits.rs — Strategy trait definition
  ├─ arbitrage.rs — Cross-outcome arbitrage detection
  └─ simple.rs — Baseline strategy implementation

execution/
  ├─ paper.rs — Paper trading executor
  └─ live.rs — Live execution (placeholder)
```

## Project Structure

```
src/
├── main.rs
├── lib.rs
├── config/
├── market_data/
│   ├── adapters/
│   │   ├── polymarket.rs
│   │   └── kalshi.rs
│   ├── router.rs
│   ├── market_worker.rs
│   └── types.rs
├── state/
│   ├── market.rs
│   ├── market_cache.rs
│   ├── position.rs
│   └── pnl.rs
├── metrics/
│   └── prometheus.rs
├── strategy/
│   ├── traits.rs
│   ├── arbitrage.rs
│   └── simple.rs
├── execution/
│   ├── paper.rs
│   └── live.rs
├── persist/
│   └── snapshot.rs
└── tests/
    └── integration.rs
ops/
├── docker-compose.yml
└── prometheus.yml
```

## Running

```bash
RUST_LOG=info cargo run
```

## Observability

The app exposes Prometheus metrics at `http://localhost:9000/metrics`.

To run Prometheus + Grafana locally:

```bash
cd ops && docker compose up -d
```

| Service    | URL                       |
|------------|---------------------------|
| Prometheus | http://localhost:9090      |
| Grafana    | http://localhost:3000      |

In Grafana, add Prometheus as a data source using `http://prometheus:9090`.

## Status

Active development — market data ingestion and real-time WebSocket streaming operational for Polymarket. Kalshi adapter, strategy execution, and persistence layers in progress.
