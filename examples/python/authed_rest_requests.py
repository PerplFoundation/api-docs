# Making a signed REST request with an API key you already have.
#
# An API key is an Ed25519 key pair. The server stores only the public key;
# every request is signed with the private key (there is no bearer token to
# leak). This example uses a key that already exists — see below for how to
# create one.
#
# Create a key via the web UI (connect your wallet):
#   Mainnet: https://app.perpl.xyz/apikeys
#   Testnet: https://testnet.perpl.xyz/apikeys
# The UI hands you the X-API-Key token and the Ed25519 private key. Export them:
#   export PERPL_API_KEY=<token>
#   export PERPL_API_KEY_SECRET=<hex of the 32-byte Ed25519 private key>
#
# To enroll a key programmatically instead, see examples/js/enroll_api_key.js.
import base64
import hashlib
import os
import secrets
import sys
import time

import requests
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey


API_URL = os.environ.get("PERPL_API_URL", "https://app.perpl.xyz/api")
PERPL_CHAIN_ID = int(os.environ.get("PERPL_CHAIN_ID", 143))
TEST_API = "/v1/trading/fills?count=1"


def _b64url(raw: bytes) -> str:
    # base64url without padding, matching the server's canonicalization.
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


def load_api_key():
    # Load an already-enrolled key from the environment:
    #   PERPL_API_KEY        — the opaque X-API-Key token
    #   PERPL_API_KEY_SECRET — hex of the 32-byte Ed25519 private key (optional 0x prefix)
    # Returns (token, priv).
    token = os.environ.get("PERPL_API_KEY")
    secret = os.environ.get("PERPL_API_KEY_SECRET")
    if not token or not secret:
        print(
            "Set PERPL_API_KEY and PERPL_API_KEY_SECRET — create a key at the web UI "
            "https://app.perpl.xyz/apikeys (testnet https://testnet.perpl.xyz/apikeys) "
            "or run the JS enrollment example examples/js/enroll_api_key.js"
        )
        sys.exit(1)

    priv = Ed25519PrivateKey.from_private_bytes(bytes.fromhex(secret.removeprefix("0x")))
    return token, priv


def signed_request(api_url, method, target, priv, token, chain_id, body=""):
    # Sign the canonical string and issue the request with the four X-API-* headers.
    method = method.upper()
    timestamp = str(int(time.time() * 1000))
    nonce = _b64url(secrets.token_bytes(16))
    body_hash = hashlib.sha256(body.encode()).hexdigest()

    # canonical = chain_id \n METHOD \n request-target \n timestamp_ms \n nonce \n hex(sha256(body))
    canonical = "\n".join([str(chain_id), method, target, timestamp, nonce, body_hash])
    sig = _b64url(priv.sign(canonical.encode()))

    headers = {
        "X-API-Key": token,
        "X-API-Timestamp": timestamp,
        "X-API-Nonce": nonce,
        "X-API-Signature": sig,
    }
    if body:
        headers["Content-Type"] = "application/json"

    return requests.request(method, api_url + target, headers=headers, data=body or None)


def signed_get(api_url, target, priv, token, chain_id, body=""):
    return signed_request(api_url, "GET", target, priv, token, chain_id, body)


def main():
    token, priv = load_api_key()

    response = signed_get(API_URL, TEST_API, priv, token, PERPL_CHAIN_ID)
    print(f"Authed Request: {API_URL + TEST_API}")
    print(f"Authed Response: {response.json()}")


if __name__ == "__main__":
    main()
