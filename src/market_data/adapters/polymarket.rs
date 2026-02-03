#![allow(warnings)] 

use tokio::sync::mpsc;

use crate::market_data::{adapters::polymarket, types::{MarketEvent, MarketEventKind, Venue}};
use std::{collections::HashMap, time::SystemTime};
use polymarket_rs::client::GammaClient;
use polymarket_rs::request::GammaMarketParams;
use polymarket_rs;
use polymarket_rs::ClobClient;
use polymarket_rs::types::TokenId;
use polymarket_rs::Side;
use polymarket_rs::websocket::MarketWsClient;
use rust_decimal::Decimal;
use polymarket_rs::StreamExt;
use polymarket_rs::types::GammaMarket;


fn is_clob_tradable(m: &GammaMarket) -> bool {
    if !m.active || m.closed || m.archived  {
        return false;
    }

    let raw_ids = match m.clob_token_ids.as_deref() {
        Some(v) => v,
        None => return false,
    };

    let raw_prices = match m.outcome_prices.as_deref() {
        Some(v) => v,
        None => return false,
    };

    let prices: Vec<f64> = serde_json::from_str::<Vec<String>>(raw_prices)  
        .ok()
        .map(|v| {
            v.into_iter()
                .filter_map(|p| p.parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default();

    // Must have at least 1 non-zero price
    let has_liquidity_signal = prices.iter().any(|p| *p > 1e-6);

    if !has_liquidity_signal {
        return false;
    }

    // Must have CLOB tokens
    let ids: Vec<String> = serde_json::from_str(raw_ids).unwrap_or_default();
    if ids.is_empty() {
        return false;
    }

    let volume24h = m.volume24hr.unwrap_or(0.0);
    if volume24h < 100000.0 {
        return false;
    }

    let liquidity = m.liquidity_num.unwrap_or(0.0);
    if liquidity < 10000.0 {
        return false;
    }

    true
}




pub async fn run_polymarket_adapter(tx: mpsc::Sender<MarketEvent>) -> anyhow::Result<()> {

    let client = GammaClient::new("https://gamma-api.polymarket.com");

    let clobClient = ClobClient::new("https://clob.polymarket.com");

    let wsClient = MarketWsClient::new();

    // Get active markets
    let params = GammaMarketParams::new()
    .with_active(true)
    .with_closed(false)
    .with_archived(false)
    .with_limit(500);


    let markets = client.get_markets(Some(params)).await?;

    println!("Total markets fetched: {}", markets.len());

    // println!("all markets are here:{:?}", &markets);

    let clob_markets: Vec<_> = markets
    .into_iter()
    .filter(|m| is_clob_tradable(m))
    .collect();

    println!("Found {} active markets", clob_markets.len());


    let mut eligibleTokenIds: Vec<String> = Vec::new();
    let mut questionTokenIds: HashMap<String, String> = HashMap::new();


    for i in 0..clob_markets.len() {

        // if clob_markets[i].clob_token_ids.is_none() {
        //     println!("{} skipped: no CLOB tokens", clob_markets[i].id);
        //     continue;
        // }

        let volume = clob_markets[i]
        .volume
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .unwrap_or(0.0);

        println!("Market {}: volume = {}", clob_markets[i].id, volume);

        let raw = clob_markets[i].clob_token_ids.as_deref().unwrap();
        let question = &clob_markets[i].question;
        let ids: Vec<String> = serde_json::from_str(raw)?;
        eligibleTokenIds.extend(ids.clone());
        questionTokenIds.insert(clob_markets[i].id.clone(), question.to_string());

        let token_id = match ids.first() {
            Some(id) => id.clone(),
            None => {
                println!("No token IDs found for market {}", clob_markets[i].id);
                continue;
            }
        };

        let event = MarketEvent {
            venue: Venue::Polymarket,
            kind: MarketEventKind::Heartbeat,
            market_id: clob_markets[i].id.clone(),
            ts_exchange_ms: Some(SystemTime::now()),
            ts_receive_ms: None,
            volume24h: Some(volume),
            last_trade_price: clob_markets[i].last_trade_price,
            liquidity: clob_markets[i].liquidity.as_ref().and_then(|l| l.parse::<f64>().ok()), 
            best_bid: clob_markets[i].best_bid,
            best_ask: clob_markets[i].best_ask,
        };

        let buy_price = match clobClient.get_price(&TokenId::from(token_id.clone()), Side::Buy).await {
            Ok(buy_price) => buy_price,
            Err(e) => {
                println!("Error fetching price: {:?}", e);
                continue;
            }
        };

        let sell_price = match clobClient.get_price(&TokenId::from(token_id), Side::Sell).await {
            Ok(sell_price) => sell_price,
            Err(e) => {
                println!("Error fetching price: {:?}", e);
                continue;
            }
        };

        println!("Market {}: Buy Price: {}, Sell Price: {}", clob_markets[i].id, buy_price.price, sell_price.price);

        if buy_price.price + sell_price.price > Decimal::from(1) {
            println!("Arbitrage opportunity detected on market {}: Buy at {}, Sell at {}", clob_markets[i].id, buy_price.price, sell_price.price);
        }

        if tx.send(event).await.is_err() {
            println!("channel closed");
        } else {
            println!("Sent event");
        }
 

        // println!("markets are here:{:?}", &markets[0..10]);
    
    }

    println!("Eligible markets for subscription: {:?}", questionTokenIds);

    println!("first token_id = {:?}", eligibleTokenIds.get(0));
    println!("len = {}", eligibleTokenIds.len());


    let mut stream = wsClient.subscribe(eligibleTokenIds).await?;

    println!("Subscribed to market updates for eligible markets.");

    while let Some(message) = stream.next().await {
        match message {
            Ok(update) => {
                println!("Received market update: {:?}", update);
            }
            Err(e) => {
                println!("Error receiving market update: {:?}", e);
            }
        }
    }

    Ok(())
}
    