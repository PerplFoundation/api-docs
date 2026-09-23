// Submitting orders over HTTP (POST /v1/trading/orders).
//
// The order submission endpoint takes a BATCH of orders and answers with one
// status per order, at the position of the order it answers. This example walks
// the whole flow a batch client needs:
//
//   1. read the wallet snapshot for the account, the head block and the last
//      request ID the account accepted,
//   2. submit a batch containing one valid order and one deliberately invalid
//      one, to show that a batch is not a transaction,
//   3. wait a block and read the open orders back, because a zero status code
//      acknowledges forwarding, not execution,
//   4. cancel the order that was placed.
//
// Requires an API key with the `trade` scope, and an account with order
// forwarding enabled (see README.md#enabling-order-forwarding-one-click-trading).
//
// This program places a REAL order. It is a post-only bid far below the mid
// price, so it should rest rather than fill, but it locks collateral until it is
// cancelled. Set PERPL_SUBMIT=1 to actually send it; without that it prints the
// batch it would have sent and exits.
use anyhow::{anyhow, Result};
use ed25519_dalek::SigningKey;
use perpl_examples::{load_api_key, signed_request_headers, DEFAULT_API_URL, DEFAULT_CHAIN_ID};
use serde_json::{json, Value};
use std::time::Duration;

// Order types (see types.md#ordertype)
const ORDER_TYPE_OPEN_LONG: u32 = 1;
const ORDER_TYPE_CANCEL: u32 = 5;
// Order flags (see types.md#orderflags)
const ORDER_FLAG_POST_ONLY: u32 = 1;

struct Client {
    http: reqwest::Client,
    api_url: String,
    chain_id: u64,
    token: String,
    signing_key: SigningKey,
}

impl Client {
    async fn get_signed(&self, target: &str) -> Result<Value> {
        let headers =
            signed_request_headers(&self.token, &self.signing_key, self.chain_id, "GET", target, b"")?;
        let mut req = self.http.get(format!("{}{}", self.api_url, target));
        for (name, value) in &headers {
            req = req.header(name, value);
        }
        Ok(req.send().await?.json().await?)
    }

    async fn get_public(&self, target: &str) -> Result<Value> {
        Ok(self
            .http
            .get(format!("{}{}", self.api_url, target))
            .send()
            .await?
            .json()
            .await?)
    }

    /// Submit a batch and report each order's own status.
    ///
    /// The HTTP status is 200 whenever the orders were judged at all, however
    /// many of them were refused — so the per-order statuses are what has to be
    /// read. A non-zero top-level `status` means the request was refused before
    /// any order was looked at, and `statuses` is then empty.
    async fn submit_batch(&self, orders: &[Value]) -> Result<Vec<Value>> {
        let target = "/v1/trading/orders";
        let body = serde_json::to_vec(&json!({ "d": orders }))?;
        let headers = signed_request_headers(
            &self.token,
            &self.signing_key,
            self.chain_id,
            "POST",
            target,
            &body,
        )?;

        let mut req = self
            .http
            .post(format!("{}{}", self.api_url, target))
            .header("Content-Type", "application/json")
            .body(body);
        for (name, value) in &headers {
            req = req.header(name, value);
        }

        let response = req.send().await?;
        let http_status = response.status();
        let reply: Value = response.json().await?;

        let batch_code = reply["status"]["code"].as_i64().unwrap_or(0);
        if batch_code != 0 {
            // Refused as a whole: nothing was forwarded. The code is repeated as
            // the HTTP status, so either can be read.
            println!(
                "batch refused ({}): {} {}",
                http_status,
                batch_code,
                reply["status"]["error"].as_str().unwrap_or("")
            );
            return Ok(vec![]);
        }

        let statuses = reply["statuses"].as_array().cloned().unwrap_or_default();
        for (i, status) in statuses.iter().enumerate() {
            // orders are identified by position, not by an echo
            let rq = orders[i]["rq"].clone();
            let code = status["code"].as_i64().unwrap_or(0);
            if code == 0 {
                println!("  [{}] rq={} accepted for forwarding", i, rq);
            } else {
                println!(
                    "  [{}] rq={} refused: {} {}",
                    i,
                    rq,
                    code,
                    status["error"].as_str().unwrap_or("")
                );
            }
        }
        Ok(statuses)
    }

    /// A zero status code acknowledges forwarding, not execution: the outcome is
    /// settled a block later. Poll the open orders until the request ID shows up.
    async fn await_order(&self, request_id: u64) -> Result<Option<Value>> {
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let snapshot = self.get_signed("/v1/trading/orders").await?;
            if let Some(orders) = snapshot["d"].as_array() {
                if let Some(order) = orders
                    .iter()
                    .find(|o| o["rq"].as_u64() == Some(request_id))
                {
                    return Ok(Some(order.clone()));
                }
            }
        }
        Ok(None)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_url = std::env::var("PERPL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    let chain_id: u64 = std::env::var("PERPL_CHAIN_ID")
        .unwrap_or_else(|_| DEFAULT_CHAIN_ID.to_string())
        .parse()?;
    let market_id: u64 = std::env::var("PERPL_MARKET_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()?;
    let order_size: f64 = std::env::var("PERPL_ORDER_SIZE")
        .unwrap_or_else(|_| "0.001".to_string())
        .parse()?;
    let submit = std::env::var("PERPL_SUBMIT").as_deref() == Ok("1");

    let (token, signing_key) = load_api_key()?;
    let client = Client {
        http: reqwest::Client::new(),
        api_url,
        chain_id,
        token,
        signing_key,
    };

    // 1. The wallet snapshot carries the account, whether it may forward orders,
    //    the last request ID it accepted, and — as `sn` — the block the trading
    //    state is current at. An HTTP client has no heartbeat stream, so this is
    //    where the head block comes from.
    let wallet = client.get_signed("/v1/trading/wallet").await?;
    let account = wallet["as"]
        .as_array()
        .and_then(|a| a.first())
        .ok_or_else(|| anyhow!("wallet holds no exchange account"))?;
    let account_id = account["id"].as_u64().unwrap_or(0);
    if !account["fw"].as_bool().unwrap_or(false) {
        return Err(anyhow!(
            "account {} may not forward orders — call allowOrderForwarding(true) on the \
             Exchange contract, see README.md#enabling-order-forwarding-one-click-trading",
            account_id
        ));
    }

    let head_block = wallet["sn"].as_u64().unwrap_or(0);
    // Request IDs must not decrease per account. Seed from `lfr`, the last
    // request ID the exchange accepted for this account.
    let mut next_request_id = account["lfr"].as_u64().unwrap_or(0) + 1;

    // The market's configuration, for the decimals and the `lb` ceiling.
    let ctx = client.get_public("/v1/pub/context").await?;
    let market = ctx["markets"]
        .as_array()
        .and_then(|markets| markets.iter().find(|m| m["id"].as_u64() == Some(market_id)))
        .ok_or_else(|| anyhow!("market {} not found in /v1/pub/context", market_id))?;
    let size_decimals = market["config"]["size_decimals"].as_u64().unwrap_or(0) as u32;
    let order_ttl_blocks = market["order_ttl_blocks"].as_u64().unwrap_or(0);

    // Mid price of the market, scaled by its price decimals. The ticker is keyed
    // by market ID the way the WebSocket stream keys it, so a single-market
    // response is a map with one entry.
    let ticker = client
        .get_public(&format!("/v1/market-data/{}/ticker", market_id))
        .await?;
    let mid = ticker["d"][market_id.to_string()]["mid"]
        .as_f64()
        .ok_or_else(|| anyhow!("no state for market {}", market_id))?;

    // A post-only bid at half the mid price: it cannot cross, so it rests.
    let price = (mid * 0.5).round() as u64;
    let size = (order_size * 10f64.powi(size_decimals as i32)).round() as u64;
    // head < lb <= head + order_ttl_blocks
    let last_exec_block = head_block + order_ttl_blocks.min(30);

    let place_request_id = next_request_id;
    next_request_id += 1;
    let batch = vec![
        json!({
            "rq": place_request_id,
            "mkt": market_id,
            "acc": account_id,
            "t": ORDER_TYPE_OPEN_LONG,
            "p": price,
            "s": size,
            "fl": ORDER_FLAG_POST_ONLY,
            "lv": 100,  // 1x, in hundredths
            "lb": last_exec_block,
        }),
        // Deliberately invalid: an unknown market. It is refused on its own and
        // the order above still stands — a batch is not a transaction.
        json!({
            "rq": next_request_id,
            "mkt": 999999,
            "acc": account_id,
            "t": ORDER_TYPE_OPEN_LONG,
            "p": price,
            "s": size,
            "fl": 0,
            "lv": 100,
            "lb": last_exec_block,
        }),
    ];
    next_request_id += 1;

    println!(
        "account {}, head block {}, mid {}",
        account_id, head_block, mid
    );
    if !submit {
        println!("PERPL_SUBMIT is not 1 — printing the batch instead of sending it:");
        println!("{}", serde_json::to_string_pretty(&json!({ "d": batch }))?);
        return Ok(());
    }

    println!("submitting batch:");
    let statuses = client.submit_batch(&batch).await?;
    if statuses.first().and_then(|s| s["code"].as_i64()) != Some(0) {
        return Ok(());
    }

    // 3. Read the outcome back. The acknowledgement above said the order was
    //    forwarded; whether it posted is settled a block later.
    let order = match client.await_order(place_request_id).await? {
        Some(order) => order,
        None => {
            println!("order did not appear in the snapshot — check the order history for a failure");
            return Ok(());
        }
    };
    println!(
        "order {} is open at {} (status {})",
        order["oid"], order["p"], order["st"]
    );

    // 4. Cancel it. One order is a batch of one.
    println!("cancelling:");
    client
        .submit_batch(&[json!({
            "rq": next_request_id,
            "mkt": market_id,
            "acc": account_id,
            "oid": order["oid"],
            "t": ORDER_TYPE_CANCEL,
            "fl": 0,
            "lv": 0,
            "lb": 0,
        })])
        .await?;

    Ok(())
}
