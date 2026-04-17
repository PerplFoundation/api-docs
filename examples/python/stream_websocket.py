# Streaming from the public websocket API without authentication
import asyncio
import json
import os
import websockets


API_URL = os.environ.get("PERPL_WS_URL", "wss://app.perpl.xyz")
WS_URL = "/ws/v1/market-data"

BTC_MAINNET = 1                                         # 16 for testnet
PERPL_CHAIN_ID = os.environ.get("PERPL_CHAIN_ID", 143)  # 10143 for testnet
HOUR = 3600

MsgTypeSubscriptionRequest = 5


async def websocket_subscribe(websocket):
    message = {
        "mt": MsgTypeSubscriptionRequest,
        "subs": [
            {"stream": f'heartbeat@{PERPL_CHAIN_ID}', "subscribe": True },
            {"stream": f'order-book@{BTC_MAINNET}', "subscribe": True },    # BTC order book (mainnet)
            {"stream": f'trades@{BTC_MAINNET}', "subscribe": True },        # BTC trades (mainnet)
            {"stream": f'candles@{BTC_MAINNET}*{HOUR}', "subscribe": True } # BTC 1h candles (mainnet)
        ]
    }
    message_json = json.dumps(message)
    await websocket.send(message_json)

    async for message in websocket:
        print(f"Received: {message}")


async def websocket_connect():
    url = API_URL + WS_URL
    async with websockets.connect(url) as websocket:
        await websocket_subscribe(websocket)


if __name__ == "__main__":
    asyncio.run(websocket_connect())
