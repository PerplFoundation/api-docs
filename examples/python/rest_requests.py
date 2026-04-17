# Making REST requests to public APIs without authentication
import os
import requests
import time

API_URL = os.environ.get("PERPL_API_URL", "https://app.perpl.xyz/api")


# Fetch exchange context
EXCHANGE_CONTEXT = "/v1/pub/context"

url = API_URL + EXCHANGE_CONTEXT
response = requests.get(url)
exchange_context = response.json()

print("Exchange Context : ", exchange_context)
print("Exchange Context Keys: ", list(exchange_context.keys()))
print()

# Fetch candles
markets = exchange_context["markets"]
btc_market = [m for m in markets if m["name"] == "BTC"][0]
btc_market_id = btc_market["id"]

# Scale for pricing, API gives integers so need to add decimal places
price_decimals = btc_market["config"]["price_decimals"]
price_scale = pow(10, price_decimals)

resolution = 3600  # Valid resolutions: 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200, 86400

n_candles = 10
to_seconds = int(time.time())
from_seconds = to_seconds - resolution * n_candles

from_ms = int(from_seconds * 1000)
to_ms = int(to_seconds * 1000)

candles_url = f"/v1/market-data/{btc_market_id}/candles/{resolution}/{from_ms}-{to_ms}"
url = API_URL + candles_url
response = requests.get(url)

candles = response.json()

print("Candles:")
for candle in candles["d"]:
    ts = candle["t"]
    open_price = candle["o"] / price_scale
    close_price = candle["c"] / price_scale
    high_price = candle["h"] / price_scale
    low_price = candle["l"] / price_scale
    volume = candle["v"]
    n_trades = candle["n"]
    print(f"ts: {ts} open_price: {open_price} close_price: {close_price} high_price: {high_price} low_price: {low_price} volume: {volume} n_trades: {n_trades}")
