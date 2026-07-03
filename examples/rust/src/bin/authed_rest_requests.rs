// Authenticating with an API key (Ed25519) and making a signed REST request.
//
// This example uses a key you already enrolled, loaded from the environment:
//   - PERPL_API_KEY        — the opaque X-API-Key token.
//   - PERPL_API_KEY_SECRET — hex of the 32-byte Ed25519 seed.
// Create a key at the web UI (https://app.perpl.xyz/apikeys mainnet /
// https://testnet.perpl.xyz/apikeys testnet), or enroll one programmatically
// with the JS example examples/js/enroll_api_key.js.
use anyhow::Result;
use perpl_examples::{load_api_key, signed_request_headers, DEFAULT_API_URL, DEFAULT_CHAIN_ID};
use serde_json::Value;

const TEST_API: &str = "/v1/trading/fills?count=1";

async fn make_authed_request(
    api_url: &str,
    chain_id: u64,
    token: &str,
    signing_key: &ed25519_dalek::SigningKey,
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", api_url, TEST_API);

    // GET with empty body; the request-target must match what the gateway sees.
    let headers = signed_request_headers(token, signing_key, chain_id, "GET", TEST_API, b"")?;

    let mut req = client.get(&url);
    for (name, value) in &headers {
        req = req.header(name, value);
    }
    let data: Value = req.send().await?.json().await?;

    println!("Authed Request: {}", url);
    println!("Authed Response: {}", data);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_url = std::env::var("PERPL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    let chain_id: u64 = std::env::var("PERPL_CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;

    let (token, signing_key) = load_api_key()?;
    make_authed_request(&api_url, chain_id, &token, &signing_key).await?;
    Ok(())
}
