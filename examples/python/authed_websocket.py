# Authenticating with the API and streaming from the trades API
import asyncio
import json
import os
import uuid
import websockets

from authed_rest_requests import perpl_auth


API_URL = os.environ.get("PERPL_API_URL", "https://app.perpl.xyz/api")
AUTH_PAYLOAD_URL = "/v1/auth/payload"
CONNECT_PAYLOAD_URL = "/v1/auth/connect"
PERPL_CHAIN_ID = int(os.environ.get("PERPL_CHAIN_ID", 143))

PERPL_WS_URL = os.environ.get("PERPL_WS_URL", "wss://app.perpl.xyz")
TRADING_API = "/ws/v1/trading"
AUTH_SIGN_IN = 4


WALLET_ADDRESS = '0xYourWalletAddress'
WALLET_KEY = '0xYourWalletPrivateKey'


async def start_stream(ws, nonce):
    ses = str(uuid.uuid4())
    message = {
        "mt": AUTH_SIGN_IN,
        "chain_id": PERPL_CHAIN_ID,
        "nonce": nonce,
        "ses": ses,
    }
    message_json = json.dumps(message)
    await ws.send(message_json)

    async for message in ws:
        print(f"Received: {message}")


async def authed_websocket(ws_url, nonce, auth_token_cookie):
    additional_headers = {
        "X-Auth-Nonce": nonce,
        "Cookie": f"auth-token={auth_token_cookie}"
    }
    url = ws_url + TRADING_API
    async with websockets.connect(url, additional_headers=additional_headers) as ws:
        await start_stream(ws, nonce)


async def main():
    nonce, auth_token_cookie = perpl_auth(API_URL, PERPL_CHAIN_ID, WALLET_ADDRESS, WALLET_KEY)
    await authed_websocket(PERPL_WS_URL, nonce, auth_token_cookie)


if __name__ == "__main__":
    asyncio.run(main())
