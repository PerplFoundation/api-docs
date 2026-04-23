use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use anyhow::Result;
use serde_json::{json, Value};

pub const DEFAULT_API_URL: &str = "https://app.perpl.xyz/api";
pub const DEFAULT_WS_URL: &str = "wss://app.perpl.xyz";
pub const DEFAULT_CHAIN_ID: u64 = 143;

const AUTH_PAYLOAD_URL: &str = "/v1/auth/payload";
const CONNECT_PAYLOAD_URL: &str = "/v1/auth/connect";

pub async fn perpl_auth(
    api_url: &str,
    chain_id: u64,
    wallet_address: &str,
    wallet_key: &str,
    ref_code: Option<&str>,
) -> Result<(String, String)> {
    let client = reqwest::Client::new();

    // Step 1: Get signing payload
    let auth_payload = json!({
        "chain_id": chain_id,
        "address": wallet_address,
    });
    let url = format!("{}{}", api_url, AUTH_PAYLOAD_URL);
    let signing_payload: Value = client
        .post(&url)
        .json(&auth_payload)
        .send()
        .await?
        .json()
        .await?;

    // Step 2: Sign the SIWE with your wallet
    let signer: PrivateKeySigner = wallet_key.parse()?;
    let message = signing_payload["message"].as_str().unwrap();
    let signature = signer.sign_message(message.as_bytes()).await?;
    let signature_hex = format!("0x{}", hex::encode(signature.as_bytes()));

    // Step 3: Connect with signature (chain_id and address required!)
    let connect_url = format!("{}{}", api_url, CONNECT_PAYLOAD_URL);
    let connect_request = json!({
        "chain_id": chain_id,
        "address": wallet_address,
        "message": message,
        "nonce": signing_payload["nonce"],
        "mac": signing_payload["mac"],
        "ref_code": ref_code.unwrap_or(""),
        "signature": signature_hex,
        "issued_at": signing_payload["issued_at"],
    });

    let response = client
        .post(&connect_url)
        .json(&connect_request)
        .send()
        .await?;
    let auth_token = response
        .cookies()
        .find(|c| c.name() == "auth-token")
        .map(|c| c.value().to_string())
        .unwrap_or_default();
    let connect_response: Value = response.json().await?;
    let nonce = connect_response["nonce"].as_str().unwrap().to_string();

    Ok((nonce, auth_token))
}
