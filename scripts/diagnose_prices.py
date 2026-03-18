#!/usr/bin/env python3
"""
Diagnostic script: Compare price data from Gamma API vs CLOB API
for markets that the arb strategy is flagging.

Usage:
    python3 scripts/diagnose_prices.py [market_id]

If no market_id is given, fetches a few high-volume markets automatically.
"""

import json
import sys
import urllib.request
import urllib.error

GAMMA_API = "https://gamma-api.polymarket.com"
CLOB_API  = "https://clob.polymarket.com"


def fetch_json(url):
    """Fetch JSON from a URL, return parsed dict/list."""
    req = urllib.request.Request(url, headers={"Accept": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        print(f"  HTTP {e.code} from {url}")
        return None
    except Exception as e:
        print(f"  Error fetching {url}: {e}")
        return None


def fetch_gamma_market(market_id):
    """Fetch a single market from the Gamma API."""
    data = fetch_json(f"{GAMMA_API}/markets?id={market_id}")
    if data and isinstance(data, list) and len(data) > 0:
        return data[0]
    return None


def fetch_clob_book(token_id):
    """Fetch the order book from the CLOB for a token."""
    return fetch_json(f"{CLOB_API}/book?token_id={token_id}")


def fetch_clob_price(token_id, side):
    """Fetch the best price from the CLOB for a token + side (buy/sell)."""
    return fetch_json(f"{CLOB_API}/price?token_id={token_id}&side={side}")


def fetch_high_volume_markets():
    """Fetch some active high-volume markets from Gamma."""
    data = fetch_json(
        f"{GAMMA_API}/markets?active=true&closed=false&archived=false"
        f"&limit=10&order=volume24hr&ascending=false"
    )
    return data or []


def analyse_market(market):
    """Analyse a single Gamma market: compare Gamma vs CLOB prices."""
    market_id = market.get("id", "?")
    question = market.get("question", "?")
    gamma_bid = market.get("best_bid")
    gamma_ask = market.get("best_ask")

    print(f"\n{'='*80}")
    print(f"Market {market_id}: {question[:70]}")
    print(f"{'='*80}")

    # --- Gamma-level data ---
    print(f"\n  [Gamma API - market level]")
    print(f"    best_bid = {gamma_bid}")
    print(f"    best_ask = {gamma_ask}")
    if gamma_bid is not None and gamma_ask is not None:
        print(f"    bid + ask = {gamma_bid + gamma_ask:.6f}  (== 1.0? {'YES' if abs(gamma_bid + gamma_ask - 1.0) < 0.001 else 'NO'})")

    # --- Token IDs ---
    raw_ids = market.get("clobTokenIds", "[]")
    raw_prices = market.get("outcomePrices", "[]")

    try:
        token_ids = json.loads(raw_ids) if isinstance(raw_ids, str) else raw_ids
    except:
        token_ids = []

    try:
        outcome_prices = json.loads(raw_prices) if isinstance(raw_prices, str) else raw_prices
    except:
        outcome_prices = []

    outcomes = market.get("outcomes", '["YES","NO"]')
    if isinstance(outcomes, str):
        try:
            outcomes = json.loads(outcomes)
        except:
            outcomes = ["YES", "NO"]

    if len(token_ids) < 2:
        print(f"  SKIP: only {len(token_ids)} tokens")
        return

    print(f"\n  [Gamma API - outcome prices]")
    for i, tid in enumerate(token_ids):
        label = outcomes[i] if i < len(outcomes) else f"token_{i}"
        price = outcome_prices[i] if i < len(outcome_prices) else "?"
        print(f"    {label} token: ...{tid[-12:]}  outcome_price = {price}")

    # --- CLOB data for each token ---
    print(f"\n  [CLOB API - per-token order book]")
    clob_data = {}

    for i, tid in enumerate(token_ids):
        label = outcomes[i] if i < len(outcomes) else f"token_{i}"
        print(f"\n    --- {label} token: ...{tid[-12:]} ---")

        # Fetch best prices
        buy_price_data = fetch_clob_price(tid, "buy")
        sell_price_data = fetch_clob_price(tid, "sell")

        buy_p = float(buy_price_data.get("price", 0)) if buy_price_data else None
        sell_p = float(sell_price_data.get("price", 0)) if sell_price_data else None

        print(f"    CLOB buy price (best ask):  {buy_p}")
        print(f"    CLOB sell price (best bid):  {sell_p}")

        if buy_p and sell_p:
            spread = buy_p - sell_p
            print(f"    spread = {spread:.4f}")

        # Fetch top of book
        book = fetch_clob_book(tid)
        if book:
            bids = book.get("bids", [])
            asks = book.get("asks", [])
            top_bid = bids[0] if bids else None
            top_ask = asks[0] if asks else None

            print(f"    Order book depth: {len(bids)} bids, {len(asks)} asks")
            if top_bid:
                print(f"    Top bid: price={top_bid.get('price')}  size={top_bid.get('size')}")
            if top_ask:
                print(f"    Top ask: price={top_ask.get('price')}  size={top_ask.get('size')}")

            clob_data[label] = {
                "best_bid": float(top_bid["price"]) if top_bid else None,
                "best_ask": float(top_ask["price"]) if top_ask else None,
            }

    # --- Comparison ---
    if len(clob_data) >= 2:
        labels = list(clob_data.keys())
        yes_data = clob_data.get("Yes") or clob_data.get("YES") or clob_data.get(labels[0])
        no_data = clob_data.get("No") or clob_data.get("NO") or clob_data.get(labels[1])

        print(f"\n  [Comparison: what arb strategy would see]")
        print(f"    Using Gamma best_bid/best_ask for BOTH tokens:")
        print(f"      YES_bid={gamma_bid}  NO_bid={gamma_bid}  sum={2*gamma_bid if gamma_bid else '?'}")
        print(f"      (This is what happens when heartbeat events use market-level values)")

        if yes_data and no_data and yes_data["best_bid"] and no_data["best_bid"]:
            yes_bid = yes_data["best_bid"]
            no_bid = no_data["best_bid"]
            yes_ask = yes_data["best_ask"]
            no_ask = no_data["best_ask"]

            print(f"\n    Using CLOB per-token best_bid/best_ask:")
            print(f"      YES_bid={yes_bid:.4f}  NO_bid={no_bid:.4f}  sum={yes_bid+no_bid:.4f}")
            print(f"      YES_ask={yes_ask:.4f}  NO_ask={no_ask:.4f}  sum={yes_ask+no_ask:.4f}")

            sell_edge = yes_bid + no_bid - 1.0
            buy_edge = 1.0 - (yes_ask + no_ask)

            print(f"\n    Sell arb edge (bid sum - 1.0):  {sell_edge:.4f}  {'<-- REAL ARB' if sell_edge > 0.005 else '(no arb)'}")
            print(f"    Buy arb edge  (1.0 - ask sum):  {buy_edge:.4f}  {'<-- REAL ARB' if buy_edge > 0.005 else '(no arb)'}")


def main():
    if len(sys.argv) > 1:
        # Specific market ID provided
        market_id = sys.argv[1]
        print(f"Fetching market {market_id} from Gamma API...")
        market = fetch_gamma_market(market_id)
        if market:
            analyse_market(market)
        else:
            print(f"Market {market_id} not found")
    else:
        # Fetch top volume markets
        print("Fetching top volume markets from Gamma API...")
        markets = fetch_high_volume_markets()
        if not markets:
            print("No markets returned")
            return

        # Analyse first 3
        for m in markets[:3]:
            analyse_market(m)

    print(f"\n{'='*80}")
    print("DIAGNOSIS COMPLETE")
    print(f"{'='*80}")


if __name__ == "__main__":
    main()
