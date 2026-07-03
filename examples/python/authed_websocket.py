# Authenticating the trading WebSocket with an API key you already have.
#
# On the trading socket, API-key auth is a signed ApiKeySignIn frame (mt: 29)
# sent as the first message after the socket opens. Key handling and the signing
# helpers are shared with the REST example.
#
# Create a key via the web UI (connect your wallet):
#   Mainnet: https://app.perpl.xyz/apikeys
#   Testnet: https://testnet.perpl.xyz/apikeys
# Then export PERPL_API_KEY and PERPL_API_KEY_SECRET (see authed_rest_requests.py).
import asyncio
import base64
import json
import os
import secrets
import time

import websockets

from authed_rest_requests import (
    API_URL,
    PERPL_CHAIN_ID,
    load_api_key,
)


PERPL_WS_URL = os.environ.get("PERPL_WS_URL", "wss://app.perpl.xyz")
TRADING_API = "/ws/v1/trading"
API_KEY_SIGN_IN = 29  # MsgTypeApiKeySignIn


def _b64url(raw: bytes) -> str:
    return base64.urlsafe_b64encode(raw).rstrip(b"=").decode()


async def start_stream(ws, priv, token, chain_id):
    # First frame: signed ApiKeySignIn. The signature covers the WS canonical
    # string: chain_id \n "trading-ws-signin" \n timestamp_ms \n nonce.
    timestamp = str(int(time.time() * 1000))
    nonce = _b64url(secrets.token_bytes(16))
    canonical = "\n".join([str(chain_id), "trading-ws-signin", timestamp, nonce])
    sig = _b64url(priv.sign(canonical.encode()))

    sign_in = {
        "mt": API_KEY_SIGN_IN,
        "chain_id": chain_id,
        "api_key": token,
        "timestamp": timestamp,
        "nonce": nonce,
        "signature": sig,
    }
    await ws.send(json.dumps(sign_in))

    async for message in ws:
        print(f"Received: {message}")


async def authed_websocket(ws_url, priv, token, chain_id):
    url = ws_url + TRADING_API
    async with websockets.connect(url) as ws:
        await start_stream(ws, priv, token, chain_id)


async def main():
    token, priv = load_api_key()
    await authed_websocket(PERPL_WS_URL, priv, token, PERPL_CHAIN_ID)


if __name__ == "__main__":
    asyncio.run(main())
