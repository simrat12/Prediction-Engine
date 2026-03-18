# Prediction Engine

A modular trading system for binary prediction markets (Polymarket, Kalshi (WIP)) built in async Rust. Streams real-time prices via WebSocket, detects cross-outcome arbitrage, and executes via a pluggable paper/live execution layer.

## Architecture

### Data Flow

```
                              Polymarket Gamma API
                              (market discovery)
                                      │
                         ┌────────────▼────────────┐
                         │  adapters/polymarket/    │
                         │                          │
                         │  init_polymarket_        │─────────────────────┐
                         │  adapter()               │                     │
                         │                          │             Returns metadata:
                         │  ┌──────────────────┐   │             MarketMap
                         │  │  ws.rs           │   │             Arc<TokenToMarket>
                         │  │  WebSocket loop  │   │             JoinHandle
                         │  │                  │   │                     │
                         │  │  BookEvent    ───┼───┼──┐                  │
                         │  │  PriceChange  ───┼───┼──┤ MarketEvent      │
                         │  └──────────────────┘   │  │ (best_bid/ask    │
                         │                          │  │  from WS JSON)  │
                         │  ┌──────────────────┐   │  │                  │
                         │  │  clob.rs         │   │  │                  │
                         │  │  Initial REST    │───┼──┘                  │
                         │  │  price fetch     │   │  MarketEvent        │
                         │  └──────────────────┘   │  (mpsc 4096)        │
                         └──────────────────────────┘                    │
                                      │                                  │
                                      ▼                                  │
                         ┌────────────────────────┐                      │
                         │       router.rs         │                      │
                         │  Routes by Venue,        │                      │
                         │  spawns per-venue        │                      │
                         │  market workers          │                      │
                         └────────────┬────────────┘                      │
                                      │ MarketEvent (mpsc 1024)           │
                                      ▼                                   │
                         ┌────────────────────────┐                       │
                         │    market_worker.rs     │                       │
                         │  Writes to cache        │                       │
                         │  Sends Notification     │                       │
                         │  (MarketKey, Instant)   │                       │
                         └──────┬─────────┬────────┘                       │
                                │         │                                │
                   ┌────────────▼──┐  Notification                         │
                   │  MarketCache  │  (mpsc 512)                           │
                   │               │       │                               │
                   │  DashMap      │       ▼                               │
                   │  <MarketKey,  │  ┌────────────────────┐              │
                   │  MarketState> │  │  strategy/mod.rs   │◄─────────────┘
                   │               │  │                    │  market_map +
                   └──────▲────────┘  │  Reads YES + NO    │  token_to_market
                          │           │  from cache via    │
                     cache.get()      │  MarketInfo lookup │
                          │           │                    │
                          └───────────┤  Runs all          │
                                      │  strategies        │
                                      └────────┬───────────┘
                                               │
                                        TradeSignal (mpsc 64)
                                        - legs[]
                                        - edge
                                        - ws_received_at
                                               │
                                               ▼
                               ┌───────────────────────────┐
                               │     execution/mod.rs      │
                               │  Signal → Intent          │
                               │  Calls executor           │
                               │  Records metrics:         │
                               │   - signal_to_fill        │
                               │   - e2e latency           │
                               │   - fills / rejections    │
                               └─────────────┬─────────────┘
                                             │
                              ┌──────────────┼──────────────┐
                              ▼                             ▼
                   ┌──────────────────┐       ┌──────────────────┐
                   │    paper.rs      │       │     live.rs      │
                   │  PaperExecutor   │       │   LiveExecutor   │
                   │  Simulated fills │       │  FOK orders via  │
                   │  Logs PAPER FILL │       │  TradingClient   │
                   └──────────────────┘       └──────────────────┘
```

### Key Types

```
MarketEvent          WS/HTTP → Router → Worker
  ├── venue            Venue::Polymarket
  ├── token_id         CLOB asset ID (each YES/NO token is separate)
  ├── market_id        Gamma market ID (groups YES + NO tokens)
  ├── received_at      Instant — monotonic, for latency measurement
  ├── best_bid/ask     Option<f64> — real top-of-book from WS or CLOB REST
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
  ├── edge                     Profit margin (e.g. 0.025 = 2.5%)
  ├── generated_at             Instant
  └── ws_received_at           Instant (from triggering WS event)

ExecutionIntent               Bridge → Executor
  ├── legs: Vec<OrderLeg>
  ├── neg_risk
  └── created_at

ExecutionReport               Executor → Bridge
  └── leg_results: Vec<LegFillStatus>   Filled | Rejected | NotAttempted
```

### Arbitrage Strategy

```
Binary market: each market has a YES token and a NO token.
At resolution, exactly one pays $1 and the other pays $0.

Sell arbitrage (bids overpriced):
  YES_bid + NO_bid > 1.0  →  sell both into existing bids
  Collect: YES_bid + NO_bid upfront
  Pay out: $1.00 at resolution (guaranteed)
  Profit:  YES_bid + NO_bid - 1.0

Buy arbitrage (asks underpriced):
  YES_ask + NO_ask < 1.0  →  buy both by hitting existing asks
  Pay:     YES_ask + NO_ask upfront
  Receive: $1.00 at resolution (guaranteed)
  Profit:  1.0 - (YES_ask + NO_ask)

Fee threshold: Polymarket charges ~1% taker fee per leg (2 legs = ~2% total).
The min_edge is set to 0.025 (2.5%) to ensure profitability net of fees,
with a small buffer for slippage. Real market edges are typically 0.002–0.004,
so signals only fire on genuine dislocations.

Both signal types emit a 2-leg TradeSignal with the correct token_ids, sides, and prices.
```

### How WS Price Data Works

Polymarket's WebSocket sends two event types:

**`BookEvent`** — Full order book snapshot (sent on connect / reconnect).
- `bids[0]` = highest bid = best bid
- `asks[0]` = lowest ask = best ask
- Used to seed the cache immediately on connection.

**`PriceChangeEvent`** — Incremental update when a price level changes.
- Each entry covers **one token** (`asset_id`) and one side of the book (`BUY` or `SELL`).
- A single event typically contains **two entries** — one for the YES token and one for the NO token — because Polymarket's CLOB is unified: placing a bid on YES at price X automatically mirrors as an ask on NO at (1−X).
- Each entry carries `best_bid` and `best_ask` — the real top-of-book values **after** this update.
- We process each entry separately using its own `asset_id` and `best_bid`/`best_ask`.

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
│   │   ├── mod.rs                   Declares active adapters
│   │   ├── polymarket/              Polymarket adapter (split by concern)
│   │   │   ├── mod.rs              Public API: init + startup orchestration
│   │   │   ├── types.rs            MarketInfo, EligibleMarket, market filter
│   │   │   ├── clob.rs             CLOB REST API price fetching
│   │   │   └── ws.rs               WebSocket reconnect loop + event handling
│   │   └── kalshi.rs                Kalshi adapter (WIP — not yet wired in)
│   ├── router.rs                    Per-venue event routing
│   └── market_worker.rs             Cache writer + strategy notifier
├── state/
│   ├── market.rs                    MarketState (bid/ask/volume)
│   └── market_cache.rs              DashMap-backed concurrent cache
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
deps/
└── polymarket-rs/                   Local patch of polymarket-rs 0.2.0
                                     Adds best_bid/best_ask to PriceChange struct
ops/
├── docker-compose.yml               Engine + Prometheus + Grafana
├── prometheus.yml                   Scrape config
├── grafana/
│   ├── provisioning/                Auto-configured datasource (no manual setup)
│   └── dashboards/                  Pre-built dashboard (12 panels)
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

| Service    | URL                  | Credentials | Purpose                     |
|------------|----------------------|-------------|-----------------------------|
| Engine     | http://localhost:9000 | —          | Prometheus metrics endpoint |
| Prometheus | http://localhost:9090 | —          | Metrics storage + queries   |
| Grafana    | http://localhost:3000 | admin/admin | Dashboards (auto-provisioned) |

Grafana is fully auto-provisioned — the Prometheus datasource and dashboard are configured automatically on first start. No manual setup required.

### Environment

| Variable      | Required  | Default | Purpose                      |
|---------------|-----------|---------|------------------------------|
| `RUST_LOG`    | No        | none    | Log level filter (e.g. info) |
| `PRIVATE_KEY` | Live only | —       | Polymarket wallet key        |

## Status

Paper trading operational — real-time Polymarket price streaming via WebSocket (BookEvent + PriceChangeEvent), cross-outcome arbitrage detection with a 2.5% minimum edge (net of fees), and paper execution with full pipeline latency tracking via Prometheus/Grafana.

Kalshi adapter, position tracking, and PnL calculation in progress.
