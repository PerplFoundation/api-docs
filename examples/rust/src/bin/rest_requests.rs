// Making REST requests to public APIs without authentication
use anyhow::Result;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_API_URL: &str = "https://app.perpl.xyz/api";

#[tokio::main]
async fn main() -> Result<()> {
    let api_url = std::env::var("PERPL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    let client = reqwest::Client::new();

    // Fetch exchange context
    let url = format!("{}/v1/pub/context", api_url);
    let exchange_context: Value = client.get(&url).send().await?.json().await?;

    println!("Exchange Context: {}", exchange_context);
    if let Some(obj) = exchange_context.as_object() {
        println!(
            "Exchange Context Keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }
    println!();

    // Fetch candles
    let markets = exchange_context["markets"].as_array().unwrap();
    let btc_market = markets
        .iter()
        .find(|m| m["name"].as_str() == Some("BTC"))
        .expect("BTC market not found");

    let btc_market_id = btc_market["id"].as_u64().unwrap();
    let price_decimals = btc_market["config"]["price_decimals"].as_u64().unwrap();
    let price_scale = 10u64.pow(price_decimals as u32) as f64;

    // Valid resolutions: 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200, 86400
    let resolution: u64 = 3600;
    let n_candles: u64 = 10;

    let to_seconds = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let from_seconds = to_seconds - resolution * n_candles;
    let from_ms = from_seconds * 1000;
    let to_ms = to_seconds * 1000;

    let candles_url = format!(
        "{}/v1/market-data/{}/candles/{}/{}-{}",
        api_url, btc_market_id, resolution, from_ms, to_ms
    );
    let candles: Value = client.get(&candles_url).send().await?.json().await?;

    println!("Candles:");
    if let Some(candle_list) = candles["d"].as_array() {
        for candle in candle_list {
            let ts = &candle["t"];
            let open_price = candle["o"].as_f64().unwrap_or(0.0) / price_scale;
            let close_price = candle["c"].as_f64().unwrap_or(0.0) / price_scale;
            let high_price = candle["h"].as_f64().unwrap_or(0.0) / price_scale;
            let low_price = candle["l"].as_f64().unwrap_or(0.0) / price_scale;
            let volume = &candle["v"];
            let n_trades = &candle["n"];
            println!(
                "ts: {} open_price: {} close_price: {} high_price: {} low_price: {} volume: {} n_trades: {}",
                ts, open_price, close_price, high_price, low_price, volume, n_trades
            );
        }
    }

    Ok(())
}
