// API-key (Ed25519) authentication helpers.
//
// An API key is an Ed25519 key pair. Enrollment is a one-time, wallet-signed
// step; afterwards every request is signed with the key's private key. These
// examples assume you already have an enrolled key and load it from the
// environment (see `load_api_key`).
//
// The easiest way to create a key is the web UI (connect your wallet):
//   - Mainnet: https://app.perpl.xyz/apikeys
//   - Testnet: https://testnet.perpl.xyz/apikeys
// To enroll programmatically, see the JS example examples/js/enroll_api_key.js.
use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use ed25519_dalek::{Signer as _, SigningKey};
use rand::RngCore;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_API_URL: &str = "https://app.perpl.xyz/api";
pub const DEFAULT_WS_URL: &str = "wss://app.perpl.xyz";
pub const DEFAULT_CHAIN_ID: u64 = 143;

/// Load a previously enrolled API key from the environment:
///   - `PERPL_API_KEY`        — the opaque `X-API-Key` token.
///   - `PERPL_API_KEY_SECRET` — hex of the 32-byte Ed25519 seed (`0x` optional).
///
/// Returns the token and the reconstructed Ed25519 signing key used to sign
/// subsequent requests.
pub fn load_api_key() -> Result<(String, SigningKey)> {
    let missing = || {
        anyhow!(
            "Set PERPL_API_KEY (token) and PERPL_API_KEY_SECRET (hex of the \
             32-byte Ed25519 seed) in the environment. Create a key at the web \
             UI https://app.perpl.xyz/apikeys (testnet \
             https://testnet.perpl.xyz/apikeys), or enroll one programmatically \
             with the JS example examples/js/enroll_api_key.js."
        )
    };

    let token = std::env::var("PERPL_API_KEY").map_err(|_| missing())?;
    let secret_hex = std::env::var("PERPL_API_KEY_SECRET").map_err(|_| missing())?;

    let seed_bytes = hex::decode(secret_hex.trim_start_matches("0x"))?;
    let seed: [u8; 32] = seed_bytes
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("PERPL_API_KEY_SECRET must be 32 bytes of hex"))?;
    let signing_key = SigningKey::from_bytes(&seed);
    Ok((token, signing_key))
}

/// base64url without padding, as required by the signature/nonce encoding.
fn b64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// current unix time in milliseconds, as a decimal string.
fn timestamp_ms() -> Result<String> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis();
    Ok(ms.to_string())
}

/// 16 random bytes, base64url (no padding).
fn random_nonce() -> String {
    let mut nonce = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    b64url(&nonce)
}

/// Signed REST headers for a request: (X-API-Key, X-API-Timestamp,
/// X-API-Nonce, X-API-Signature).
///
/// `target` is the request-target (path + query) exactly as the gateway
/// receives it. `body` is the raw request body bytes ("" for GET).
pub fn signed_request_headers(
    token: &str,
    signing_key: &SigningKey,
    chain_id: u64,
    method: &str,
    target: &str,
    body: &[u8],
) -> Result<[(String, String); 4]> {
    let timestamp = timestamp_ms()?;
    let nonce = random_nonce();
    let body_hash = hex::encode(Sha256::digest(body));

    let canonical = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        chain_id, method, target, timestamp, nonce, body_hash
    );
    let signature = b64url(&signing_key.sign(canonical.as_bytes()).to_bytes());

    Ok([
        ("X-API-Key".to_string(), token.to_string()),
        ("X-API-Timestamp".to_string(), timestamp),
        ("X-API-Nonce".to_string(), nonce),
        ("X-API-Signature".to_string(), signature),
    ])
}

/// Build the signed WebSocket sign-in frame (mt: 29) for `/ws/v1/trading`.
/// Returns the JSON string to send as the first text frame after connecting.
pub fn ws_signin_frame(token: &str, signing_key: &SigningKey, chain_id: u64) -> Result<String> {
    let timestamp = timestamp_ms()?;
    let nonce = random_nonce();

    let canonical = format!("{}\ntrading-ws-signin\n{}\n{}", chain_id, timestamp, nonce);
    let signature = b64url(&signing_key.sign(canonical.as_bytes()).to_bytes());

    let frame = json!({
        "mt": 29,
        "chain_id": chain_id,
        "api_key": token,
        "timestamp": timestamp,
        "nonce": nonce,
        "signature": signature,
    });
    Ok(frame.to_string())
}
