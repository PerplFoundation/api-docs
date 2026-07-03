// Authenticating with an API key (Ed25519) and streaming from the trading WS.
//
// This example uses a key you already enrolled, loaded from the environment:
//   - PERPL_API_KEY        — the opaque X-API-Key token.
//   - PERPL_API_KEY_SECRET — hex of the 32-byte Ed25519 seed.
// Create a key at the web UI (https://app.perpl.xyz/apikeys mainnet /
// https://testnet.perpl.xyz/apikeys testnet), or enroll one programmatically
// with the JS example examples/js/enroll_api_key.js.
use anyhow::anyhow;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use http::Request;
use perpl_examples::{load_api_key, ws_signin_frame, DEFAULT_CHAIN_ID, DEFAULT_WS_URL};
use tokio_tungstenite::tungstenite::handshake::client::generate_key;
use tokio_tungstenite::tungstenite::Message;
use url::Url;

async fn authed_websocket(
    ws_url: &str,
    chain_id: u64,
    token: &str,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<()> {
    let url = format!("{}/ws/v1/trading", ws_url);
    let url_parsed = Url::parse(url.as_str())?;
    let url_host = url_parsed
        .host()
        .ok_or_else(|| anyhow!("no host in url"))?
        .to_string();
    let url_port = url_parsed.port().unwrap_or(443);
    let host_header = format!("{}:{}", url_host, url_port);

    let request = Request::builder()
        .uri(url)
        .header("Host", host_header)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .body(())?;

    let (mut ws, _) = tokio_tungstenite::connect_async(request).await?;

    // First frame after connect: the signed API-key sign-in (mt: 29).
    let sign_in = ws_signin_frame(token, signing_key, chain_id)?;
    ws.send(Message::Text(sign_in)).await?;

    while let Some(msg) = ws.next().await {
        println!("Received: {}", msg?);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let ws_url = std::env::var("PERPL_WS_URL").unwrap_or_else(|_| DEFAULT_WS_URL.to_string());
    let chain_id: u64 = std::env::var("PERPL_CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;

    let (token, signing_key) = load_api_key()?;
    authed_websocket(&ws_url, chain_id, &token, &signing_key).await?;
    Ok(())
}
