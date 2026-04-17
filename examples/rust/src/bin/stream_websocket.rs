// Streaming from the public websocket API without authentication
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

const BTC_MAINNET: u64 = 1; // 16 for testnet
const DEFAULT_CHAIN_ID: u64 = 143; // 10143 for testnet
const HOUR: u64 = 3600;

const MSG_TYPE_SUBSCRIPTION_REQUEST: u64 = 5;

async fn websocket_subscribe(
    ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>,
    chain_id: u64,
) -> Result<()> {
    let message = json!({
        "mt": MSG_TYPE_SUBSCRIPTION_REQUEST,
        "subs": [
            {"stream": format!("heartbeat@{}", chain_id), "subscribe": true},
            {"stream": format!("order-book@{}", BTC_MAINNET), "subscribe": true},   // BTC order book (mainnet)
            {"stream": format!("trades@{}", BTC_MAINNET), "subscribe": true},       // BTC trades (mainnet)
            {"stream": format!("candles@{}*{}", BTC_MAINNET, HOUR), "subscribe": true}, // BTC 1h candles (mainnet)
        ]
    });
    ws.send(Message::Text(message.to_string())).await?;

    while let Some(msg) = ws.next().await {
        println!("Received: {}", msg?);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let base_url =
        std::env::var("PERPL_WS_URL").unwrap_or_else(|_| "wss://app.perpl.xyz".to_string());
    let chain_id: u64 = std::env::var("PERPL_CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;
    let url = format!("{}/ws/v1/market-data", base_url);

    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await?;
    websocket_subscribe(&mut ws, chain_id).await?;
    Ok(())
}
