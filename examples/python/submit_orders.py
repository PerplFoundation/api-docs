# Submitting orders over HTTP (POST /v1/trading/orders).
#
# The order submission endpoint takes a BATCH of orders and answers with one
# status per order, at the position of the order it answers. This example walks
# the whole flow a batch client needs:
#
#   1. read the wallet snapshot for the account, the head block and the last
#      request ID the account accepted,
#   2. submit a batch containing one valid order and one deliberately invalid
#      one, to show that a batch is not a transaction,
#   3. wait a block and read the open orders back, because a zero status code
#      acknowledges forwarding, not execution,
#   4. cancel the order that was placed.
#
# Requires an API key with the `trade` scope, and an account with order
# forwarding enabled (see README.md#enabling-order-forwarding-one-click-trading).
#
# This program places a REAL order. It is a post-only bid far below the mid
# price, so it should rest rather than fill, but it locks collateral until it is
# cancelled. Set PERPL_SUBMIT=1 to actually send it; without that it prints the
# batch it would have sent and exits.
import json
import os
import time

import requests

from authed_rest_requests import API_URL, PERPL_CHAIN_ID, load_api_key, signed_request


MARKET_ID = int(os.environ.get("PERPL_MARKET_ID", 1))
ORDER_SIZE = float(os.environ.get("PERPL_ORDER_SIZE", 0.001))
SUBMIT = os.environ.get("PERPL_SUBMIT") == "1"

# Order types (see types.md#ordertype)
ORDER_TYPE_OPEN_LONG = 1
ORDER_TYPE_CANCEL = 5
# Order flags (see types.md#orderflags)
ORDER_FLAG_POST_ONLY = 1


def get_json(target, priv, token):
    response = signed_request(API_URL, "GET", target, priv, token, PERPL_CHAIN_ID)
    response.raise_for_status()
    return response.json()


def get_market(market_id):
    # The market's configuration, for the decimals and the `lb` ceiling.
    ctx = requests.get(f"{API_URL}/v1/pub/context").json()
    for market in ctx["markets"]:
        if market["id"] == market_id:
            return market
    raise RuntimeError(f"market {market_id} not found in /v1/pub/context")


def get_mid_price(market_id):
    # Mid price of the market, scaled by its price decimals. The ticker is keyed
    # by market ID the way the WebSocket stream keys it, so a single-market
    # response is a map with one entry.
    ticker = requests.get(f"{API_URL}/v1/market-data/{market_id}/ticker").json()
    state = ticker["d"].get(str(market_id))
    if not state:
        raise RuntimeError(f"no state for market {market_id}")
    return state["mid"]


def submit_batch(orders, priv, token):
    # Submit a batch and report each order's own status.
    #
    # The HTTP status is 200 whenever the orders were judged at all, however many
    # of them were refused — so the per-order statuses are what has to be read. A
    # non-zero top-level `status` means the request was refused before any order
    # was looked at, and `statuses` is then empty.
    body = json.dumps({"d": orders})
    response = signed_request(API_URL, "POST", "/v1/trading/orders", priv, token, PERPL_CHAIN_ID, body)
    reply = response.json()

    if reply.get("status", {}).get("code"):
        # Refused as a whole: nothing was forwarded. The code is repeated as the
        # HTTP status, so either can be read.
        status = reply["status"]
        print(f"batch refused ({response.status_code}): {status['code']} {status.get('error')}")
        return []

    statuses = reply.get("statuses", [])
    for i, status in enumerate(statuses):
        order = orders[i]  # orders are identified by position, not by an echo
        if status["code"] == 0:
            print(f"  [{i}] rq={order['rq']} accepted for forwarding")
        else:
            print(f"  [{i}] rq={order['rq']} refused: {status['code']} {status.get('error')}")
    return statuses


def await_order(request_id, priv, token, attempts=10):
    # A zero status code acknowledges forwarding, not execution: the outcome is
    # settled a block later. Poll the open orders until the request ID shows up.
    for _ in range(attempts):
        time.sleep(1)
        snapshot = get_json("/v1/trading/orders", priv, token)
        for order in snapshot["d"]:
            if order["rq"] == request_id:
                return order
    return None


def main():
    token, priv = load_api_key()

    # 1. The wallet snapshot carries the account, whether it may forward orders,
    #    the last request ID it accepted, and — as `sn` — the block the trading
    #    state is current at. An HTTP client has no heartbeat stream, so this is
    #    where the head block comes from.
    wallet = get_json("/v1/trading/wallet", priv, token)
    accounts = wallet.get("as") or []
    if not accounts:
        raise RuntimeError("wallet holds no exchange account")
    account = accounts[0]
    if not account.get("fw"):
        raise RuntimeError(
            f"account {account['id']} may not forward orders — call allowOrderForwarding(true) "
            "on the Exchange contract, see README.md#enabling-order-forwarding-one-click-trading"
        )

    head_block = wallet["sn"]
    # Request IDs must not decrease per account. Seed from `lfr`, the last
    # request ID the exchange accepted for this account.
    next_request_id = account["lfr"] + 1

    market = get_market(MARKET_ID)
    mid = get_mid_price(MARKET_ID)

    # A post-only bid at half the mid price: it cannot cross, so it rests.
    price = round(mid * 0.5)
    size = round(ORDER_SIZE * 10 ** market["config"]["size_decimals"])
    # head < lb <= head + order_ttl_blocks
    last_exec_block = head_block + min(30, market["order_ttl_blocks"])

    place_request_id = next_request_id
    next_request_id += 1
    batch = [
        {
            "rq": place_request_id,
            "mkt": MARKET_ID,
            "acc": account["id"],
            "t": ORDER_TYPE_OPEN_LONG,
            "p": price,
            "s": size,
            "fl": ORDER_FLAG_POST_ONLY,
            "lv": 100,  # 1x, in hundredths
            "lb": last_exec_block,
        },
        {
            # Deliberately invalid: an unknown market. It is refused on its own
            # and the order above still stands — a batch is not a transaction.
            "rq": next_request_id,
            "mkt": 999999,
            "acc": account["id"],
            "t": ORDER_TYPE_OPEN_LONG,
            "p": price,
            "s": size,
            "fl": 0,
            "lv": 100,
            "lb": last_exec_block,
        },
    ]
    next_request_id += 1

    print(f"account {account['id']}, head block {head_block}, mid {mid}")
    if not SUBMIT:
        print("PERPL_SUBMIT is not 1 — printing the batch instead of sending it:")
        print(json.dumps({"d": batch}, indent=2))
        return

    print("submitting batch:")
    statuses = submit_batch(batch, priv, token)
    if not statuses or statuses[0]["code"] != 0:
        return

    # 3. Read the outcome back. The acknowledgement above said the order was
    #    forwarded; whether it posted is settled a block later.
    order = await_order(place_request_id, priv, token)
    if not order:
        print("order did not appear in the snapshot — check the order history for a failure")
        return
    print(f"order {order['oid']} is open at {order['p']} (status {order['st']})")

    # 4. Cancel it. One order is a batch of one.
    print("cancelling:")
    submit_batch(
        [
            {
                "rq": next_request_id,
                "mkt": MARKET_ID,
                "acc": account["id"],
                "oid": order["oid"],
                "t": ORDER_TYPE_CANCEL,
                "fl": 0,
                "lv": 0,
                "lb": 0,
            }
        ],
        priv,
        token,
    )


if __name__ == "__main__":
    main()
