# Prediction Engine

A modular trading system for binary prediction markets (Polymarket, Kalshi (TODO)) built in async Rust. Streams real-time prices via WebSocket, detects cross-outcome arbitrage, and executes via a pluggable paper/live execution layer.

## Architecture

### Data Flow

```
                                  Polymarket
                                  Gamma API
                                     │
                            ┌────────▼────────┐
                            │  polymarket.rs   │
                            │                  │
                            │  init_polymarket │──────────────────────┐
                            │  _adapter()      │                      │
                            │                  │              Returns metadata:
                            │  ┌────────────┐  │              MarketMap
                            │  │  WS Loop   │  │              Arc<TokenToMarket>
                            │  │            │  │              JoinHandle
                            │  │ Instant::  │  │                      │
                            │  │ now() ──┐  │  │                      │
                            │  └─────────┼──┘  │                      │
                            └────────────┼─────┘                      │
                                         │                            │
                              MarketEvent│(with received_at)          │
                              (mpsc 4096)│                            │
                                         ▼                            │
                            ┌─────────────────┐                       │
                            │    router.rs     │                       │
                            │                  │                       │
                            │ Routes by Venue  │                       │
                            │ Spawns per-venue │                       │
                            │ market workers   │                       │
                            └────────┬─────────┘                       │
                                     │                                 │
                              MarketEvent                              │
                              (mpsc 1024)                              │
                                     ▼                                 │
                            ┌─────────────────┐                        │
                            │ market_worker.rs │                        │
                            │                  │                        │
                            │ Writes to cache  │                        │
                            │ Sends Notification                        │
                            │ (MarketKey,      │                        │
                            │  Instant)        │                        │
                            └──┬──────────┬────┘                        │
                               │          │                             │
                    ┌──────────▼──┐   Notification                      │
                    │ MarketCache │   (mpsc 512)                        │
                    │             │       │                              │
                    │ DashMap     │       │                              │
                    │ <(Venue,    │       ▼                              │
                    │  token_id), │  ┌─────────────────┐                │
                    │  MarketState│  │  strategy/mod.rs │◄───────────────┘
                    │ >           │  │                  │   market_map +
                    │             │  │ Reads YES + NO   │   token_to_market
                    └──────▲──────┘  │ from cache via   │
                           │         │ MarketInfo lookup │
                           │         │                  │
                      cache.get()    │ Runs all         │
                           │         │ strategies       │
                           └─────────┤                  │
                                     └────────┬─────────┘
                                              │
                                       TradeSignal
                                       (mpsc 64)
                                       - legs[]
                                       - edge
                                       - ws_received_at
                                              │
                                              ▼
                                ┌──────────────────────┐
                                │  execution/mod.rs    │
                                │  run_execution_      │
                                │  bridge()            │
                                │                      │
                                │  Signal → Intent     │
                                │  Calls executor      │
                                │  Records metrics:    │
                                │   - signal_to_fill   │
                                │   - e2e latency      │
                                │   - fills/rejections │
                                └──────────┬───────────┘
                                           │
                             ┌─────────────┼─────────────┐
                             ▼                           ▼
                  ┌──────────────────┐       ┌──────────────────┐
                  │   paper.rs       │       │    live.rs       │
                  │   PaperExecutor  │       │   LiveExecutor   │
                  │                  │       │                  │
                  │ Simulates fills  │       │ FOK orders via   │
                  │ Logs PAPER FILL  │       │ TradingClient    │
                  │ Atomic order IDs │       │ Sequential legs  │
                  └──────────────────┘       └──────────────────┘
```

### Key Types

```
MarketEvent          WS/HTTP → Router → Worker
  ├── venue            Venue::Polymarket
  ├── token_id         CLOB asset ID (each YES/NO token is separate)
  ├── market_id        Gamma market ID (groups YES + NO)
  ├── received_at      Instant — monotonic, for latency measurement
  ├── best_bid/ask     Option<f64>
  └── volume24h        Option<f64>

MarketKey(Venue, token_id)    Cache key — one entry per outcome token

MarketInfo                    Static metadata per market
  ├── yes_token_id
  ├── no_token_id
  └── neg_risk

EvalContext                   Passed to strategies each tick
  ├── updated_key/state       The token that just changed
  ├── cache                   Full DashMap read access
  ├── market_map              market_id → MarketInfo
  ├── token_to_market         token_id → market_id
  └── ws_received_at          For e2e latency tracking

TradeSignal                   Strategy output → Execution bridge
  ├── legs: Vec<SignalLeg>     [{token_id, side, price, size}, ...]
  ├── edge                     Profit margin (e.g. 0.02 = 2%)
  ├── generated_at             Instant
  └── ws_received_at           Instant (from triggering WS event)

ExecutionIntent               Bridge → Executor
  ├── legs: Vec<OrderLeg>
  ├── neg_risk
  └── created_at

ExecutionReport               Executor → Bridge
  └── leg_results: Vec<LegFillStatus>   Filled | Rejected | NotAttempted
```

### Arbitrage Logic

```
Binary market: each market has a YES token and a NO token.

Sell arbitrage (overpriced):
  YES_bid + NO_bid > 1.0  →  sell both  →  guaranteed profit = sum - 1.0

Buy arbitrage (underpriced):
  YES_ask + NO_ask < 1.0  →  buy both   →  guaranteed profit = 1.0 - sum

Both emit a 2-leg TradeSignal with the correct token_ids, sides, and prices.
```

### Metrics (Prometheus on :9000/metrics)

```
adapter_events_total          {venue, event_type}        Counter
adapter_event_latency_ms      {venue, event_type}        Histogram
strategy_signals_total        {strategy, venue}          Counter
strategy_signal_edge          {strategy}                 Histogram
execution_fills_total         {strategy, executor}       Counter
execution_rejections_total    {strategy, executor}       Counter
execution_signal_to_fill_us   {strategy}                 Histogram
execution_e2e_latency_us      {strategy}                 Histogram
```

## Project Structure

```
src/
├── main.rs                          Entry point — wires channels, spawns tasks
├── lib.rs                           Crate root — exports all modules
├── config/
│   └── mod.rs                       Environment config
├── market_data/
│   ├── types.rs                     MarketEvent, Venue, Side, MarketEventKind
│   ├── adapters/
│   │   ├── polymarket.rs            Gamma API + CLOB WebSocket adapter
│   │   └── kalshi.rs                Kalshi adapter (stub)
│   ├── router.rs                    Per-venue event routing
│   └── market_worker.rs             Cache writer + strategy notifier
├── state/
│   ├── market.rs                    MarketState (bid/ask/volume)
│   └── market_cache.rs             DashMap-backed concurrent cache
├── strategy/
│   ├── traits.rs                    Strategy trait, TradeSignal, EvalContext
│   ├── arbitrage.rs                 Cross-outcome arbitrage strategy
│   └── mod.rs                       Strategy engine loop
├── execution/
│   ├── traits.rs                    ExecutionEngine trait, Intent/Report types
│   ├── paper.rs                     PaperExecutor (simulated fills)
│   ├── live.rs                      LiveExecutor (Polymarket CLOB via FOK)
│   └── mod.rs                       Signal → execution bridge + metrics
└── metrics/
    ├── mod.rs                       Metrics init
    └── prometheus.rs                Prometheus counters + histograms
ops/
├── docker-compose.yml               Engine + Prometheus + Grafana
├── prometheus.yml                   Scrape config
Dockerfile                           Multi-stage build (builder + slim runtime)
```

## Running

### Local

```bash
RUST_LOG=info cargo run --release
```

### Docker (24/7 with observability)

```bash
cd ops && docker compose up -d --build
```

| Service    | URL                       | Purpose                        |
|------------|---------------------------|--------------------------------|
| Engine     | http://localhost:9000      | Prometheus metrics endpoint    |
| Prometheus | http://localhost:9090      | Metrics storage + queries      |
| Grafana    | http://localhost:3000      | Dashboards (admin/admin)       |

In Grafana, add Prometheus as a data source at `http://prometheus:9090`.

### Environment

| Variable      | Required | Default | Purpose                      |
|---------------|----------|---------|------------------------------|
| `RUST_LOG`    | No       | none    | Log level filter (e.g. info) |
| `PRIVATE_KEY` | Live only| —       | Polymarket wallet key        |

## Status

Paper trading operational — real-time Polymarket price streaming, cross-outcome arbitrage detection, paper execution with full pipeline latency tracking. Kalshi adapter, position tracking, and PnL calculation in progress.
