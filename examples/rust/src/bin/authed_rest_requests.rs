// Authenticating with the API and making a REST request
use anyhow::Result;
use perpl_examples::{perpl_auth, DEFAULT_API_URL, DEFAULT_CHAIN_ID};
use serde_json::Value;

const TEST_API: &str = "/v1/profile/contact-info";

const WALLET_ADDRESS: &str = "0xYourWalletAddress";
const WALLET_KEY: &str = "0xYourWalletPrivateKey";

async fn make_authed_request(api_url: &str, nonce: &str, auth_token_cookie: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", api_url, TEST_API);
    let data: Value = client
        .get(&url)
        .header("X-Auth-Nonce", nonce)
        .header("Cookie", format!("auth-token={}", auth_token_cookie))
        .send()
        .await?
        .json()
        .await?;

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

    let (nonce, auth_token_cookie) =
        perpl_auth(&api_url, chain_id, WALLET_ADDRESS, WALLET_KEY).await?;
    make_authed_request(&api_url, &nonce, &auth_token_cookie).await?;
    Ok(())
}
