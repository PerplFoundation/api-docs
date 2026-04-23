// Authenticating with the API and streaming from the trades API
use anyhow::anyhow;
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use http::Request;
use perpl_examples::{perpl_auth, DEFAULT_API_URL, DEFAULT_CHAIN_ID, DEFAULT_WS_URL};
use serde_json::json;
use tokio_tungstenite::tungstenite::handshake::client::generate_key;
use tokio_tungstenite::tungstenite::Message;
use url::Url;
use uuid::Uuid;

const AUTH_SIGN_IN: u64 = 4;

const WALLET_ADDRESS: &str = "0xYourWalletAddress";
const WALLET_KEY: &str = "0xYourWalletPrivateKey";

async fn authed_websocket(ws_url: &str, nonce: &str, auth_token_cookie: &str) -> Result<()> {
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
        .header("X-Auth-Nonce", nonce)
        .header("Cookie", format!("auth-token={}", auth_token_cookie))
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .body(())?;

    let (mut ws, _) = tokio_tungstenite::connect_async(request).await?;

    let ses = Uuid::new_v4().to_string();
    let chain_id: u64 = std::env::var("CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;

    let sign_in = json!({
        "mt": AUTH_SIGN_IN,
        "chain_id": chain_id,
        "nonce": nonce,
        "ses": ses,
    });
    ws.send(Message::Text(sign_in.to_string())).await?;

    while let Some(msg) = ws.next().await {
        println!("Received: {}", msg?);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_url = std::env::var("PERPL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    let ws_url = std::env::var("PERPL_WS_URL").unwrap_or_else(|_| DEFAULT_WS_URL.to_string());
    let chain_id: u64 = std::env::var("PERPL_CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;
    let ref_code: Option<String> = std::env::var("PERPL_REF_CODE").ok();

    let (nonce, auth_token_cookie) =
        perpl_auth(&api_url, chain_id, WALLET_ADDRESS, WALLET_KEY, ref_code.as_deref()).await?;
    authed_websocket(&ws_url, &nonce, &auth_token_cookie).await?;
    Ok(())
}
