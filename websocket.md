# WebSocket API

Perpl provides two WebSocket endpoints for real-time data.

## Endpoints

| Endpoint | Purpose | Authentication |
|----------|---------|----------------|
| `/ws/v1/market-data` | Public market data | None |
| `/ws/v1/trading` | Trading & account data | Required |

**URLs** (configurable via `PERPL_WS_URL`, default: `wss://app.perpl.xyz`):
- Market Data: `${PERPL_WS_URL}/ws/v1/market-data`
- Trading: `${PERPL_WS_URL}/ws/v1/trading`

## Message Format

All messages are JSON with a common header:

```typescript
interface MessageHeader {
  mt: number;       // Message type
  sid?: number;     // Subscription ID
  sn?: number;      // Sequence number
  cid?: number;     // Correlation ID
  ses?: string;     // Session ID
}
```

## Message Types

| Value | Name | Direction |
|-------|------|-----------|
| 1 | Ping | Client → Server |
| 2 | Pong | Server → Client |
| 3 | StatusResponse | Server → Client |
| 5 | SubscriptionRequest | Client → Server |
| 6 | SubscriptionResponse | Server → Client |
| 7 | GasPriceUpdate | Server → Client |
| 8 | MarketConfigUpdate | Server → Client |
| 9 | MarketStateUpdate | Server → Client |
| 10 | MarketFundingUpdate | Server → Client |
| 11 | CandlesSnapshot | Server → Client |
| 12 | CandlesUpdate | Server → Client |
| 15 | L2BookSnapshot | Server → Client |
| 16 | L2BookUpdate | Server → Client |
| 17 | TradesSnapshot | Server → Client |
| 18 | TradesUpdate | Server → Client |
| 19 | WalletSnapshot | Server → Client |
| 20 | WalletUpdate | Server → Client |
| 21 | AccountUpdate | Server → Client |
| 22 | OrderRequest | Client → Server |
| 23 | OrdersSnapshot | Server → Client |
| 24 | OrdersUpdate | Server → Client |
| 25 | FillsUpdate | Server → Client |
| 26 | PositionsSnapshot | Server → Client |
| 27 | PositionsUpdate | Server → Client |
| 28 | AccountStatsUpdate | Server → Client |
| 29 | ApiKeySignIn | Client → Server |
| 100 | Heartbeat | Server → Client |

---

## Market Data WebSocket

### Connecting

```typescript
const WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';
const ws = new WebSocket(`${WS_URL}/ws/v1/market-data`);
```

### Available Streams

| Stream | Format | Description |
|--------|--------|-------------|
| heartbeat | `heartbeat@<chain_id>` | Block sync heartbeat |
| gas-stats | `gas-stats@<chain_id>` | Gas price updates |
| market-config | `market-config@<chain_id>` | Market configuration |
| market-state | `market-state@<chain_id>` | Prices, volume, OI |
| funding | `funding@<chain_id>` | Funding rate updates (see [FundingEvent](./types.md#fundingevent): two messages per interval, keyed by `feb`) |
| candles | `candles@<market_id>*<resolution>` | OHLCV candles |
| order-book | `order-book@<market_id>` | L2 order book |
| trades | `trades@<market_id>` | Recent trades |

**Chain ID**: Configurable via `PERPL_CHAIN_ID` (default: 143 for Monad Mainnet)

**Candle Resolutions** (seconds): 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200, 86400

### Subscribing

```typescript
// Subscribe to streams
ws.send(JSON.stringify({
  mt: 5,  // SubscriptionRequest
  subs: [
    { stream: 'heartbeat@143', subscribe: true },
    { stream: 'order-book@1', subscribe: true },     // BTC order book (mainnet)
    { stream: 'trades@1', subscribe: true },         // BTC trades (mainnet)
    { stream: 'candles@1*3600', subscribe: true }    // BTC 1h candles (mainnet)
  ]
}));
```

The market-data server allows **16 subscriptions** and **10 requests/min** per
connection — lower than the trading server, and the same on testnet and mainnet.
Batch multiple streams into one `mt: 5` frame rather than sending one per stream.
The two limits fail differently: exceeding the request rate closes the socket with
`1008`, while exceeding the subscription cap only fails that entry in the response
(see below). See [Rate Limits](./README.md#rate-limits).

### Subscription Response

```typescript
interface SubscriptionResponse {
  mt: 6;
  subs: Array<{
    stream: string;
    sid?: number;      // Subscription ID (use to match updates)
    status?: {
      code: number;    // 0 = success; 429 = too many subscriptions; 404 = unknown stream
      error?: string;
    };
  }>;
}
```

Failures here are **per-subscription, not per-connection** — the socket stays open
and the other entries in the same request still succeed. Check `status.code` on
every element rather than assuming the whole batch applied:

| `code` | `error` | Recovery |
|---|---|---|
| 429 | `too many subscriptions` | Subscription cap reached; unsubscribe from a stream and retry |
| 404 | `unknown stream` | Bad stream name or market ID; fix the identifier |

### Order Book Messages

**Snapshot** (mt: 15):
```typescript
interface L2Book {
  mt: 15;
  sid: number;
  at: BlockTimestamp;
  bid: L2PriceLevel[];  // Bids (best to worst)
  ask: L2PriceLevel[];  // Asks (best to worst)
}

interface L2PriceLevel {
  p: number;  // Price (scaled by price_decimals)
  s: number;  // Size (scaled by size_decimals)
  o: number;  // Number of orders
}
```

**Update** (mt: 16):
Same structure. Price levels with `o: 0` should be removed.

### Trade Messages

**Snapshot** (mt: 17):
```typescript
interface TradeSeries {
  mt: 17;
  sid: number;
  d: Trade[];
}

interface Trade {
  at: BlockTxLogTimestamp;
  p: number;       // Price (scaled)
  s: number;       // Size (scaled)
  sd: TradeSide;   // 1=Buy, 2=Sell
}
```

**Update** (mt: 18):
Same structure, contains new trades.

### Candle Messages

**Snapshot** (mt: 11):
```typescript
interface CandleSeries {
  mt: 11;
  sid: number;
  at: BlockTimestamp;
  r: number;     // Resolution (seconds)
  d: Candle[];   // Candles (oldest to newest)
}
```

**Update** (mt: 12):
Contains up to 2 candles: previous (closed) and current (updated).

### Market State Update (mt: 9)

```typescript
interface MarketStateUpdate {
  mt: 9;
  d: Record<MarketID, MarketState | undefined>;
}

interface MarketState {
  at: BlockTimestamp;
  orl: number;   // Oracle price
  mrk: number;   // Mark price
  lst: number;   // Last price
  mid: number;   // Mid price
  bid: number;   // Best bid
  ask: number;   // Best ask
  prv: number;   // Price 24h ago
  dv: number;    // Daily volume (size)
  dva: string;   // Daily volume (amount)
  oi: number;    // Open interest
  tvl: string;   // Total value locked
}
```

### Heartbeat (mt: 100)

```typescript
interface Heartbeat {
  mt: 100;
  sn: number;  // Sequence number (strictly +1 from previous)
  h: number;   // Latest head block number
}
```

---

## Trading WebSocket

### Connecting & Authenticating

The primary way to authenticate the trading WebSocket is **API-key sign-in**
(`mt: 29`). API keys are Ed25519 key pairs created at the web UI
(https://app.perpl.xyz/apikeys for mainnet, https://testnet.perpl.xyz/apikeys
for testnet) or programmatically (see [Integrations](./integrations.md)).
Placing orders requires a `trade`-scoped key — a `read`-scoped key still receives
snapshots/updates, but its `OrderRequest` frames are rejected with `403`.

#### API-key sign-in (mt: 29)

Send an `ApiKeySignIn` frame as the **first** message after the socket opens, and
send it promptly — the sign-in frame must arrive within the server's idle timeout
(5s on mainnet, 10s on testnet) or the connection is closed with `1008`
(`idle timeout`).

The Ed25519 signature covers the WS canonical string — four fields joined by
`\n` (newline):

```
<chain_id>
trading-ws-signin      literal action tag
<timestamp_ms>         unix epoch milliseconds, decimal string
<nonce>                client-random, base64url (no padding)
```

Frame shape:

```typescript
{
  mt: 29,               // MsgTypeApiKeySignIn
  chain_id: number,
  api_key: string,      // X-API-Key token from enrollment
  timestamp: string,    // unix ms, decimal
  nonce: string,        // client-random, base64url
  signature: string,    // base64url(ed25519 signature over the canonical string)
}
```

```typescript
import { randomBytes } from 'crypto';
import * as ed from '@noble/ed25519';

const WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

const ws = new WebSocket(`${WS_URL}/ws/v1/trading`);

ws.onopen = async () => {
  const timestamp = Date.now().toString();
  const nonce = randomBytes(16).toString('base64url');
  const canonical = [CHAIN_ID, 'trading-ws-signin', timestamp, nonce].join('\n');
  const sig = await ed.signAsync(Buffer.from(canonical), privateKey);

  // Must authenticate immediately, as the first frame
  ws.send(JSON.stringify({
    mt: 29,             // MsgTypeApiKeySignIn
    chain_id: CHAIN_ID,
    api_key: API_KEY,   // X-API-Key token from enrollment
    timestamp,
    nonce,
    signature: Buffer.from(sig).toString('base64url'),
  }));
};
```

### Initial Snapshots

After authentication, you receive snapshots:

1. **WalletSnapshot** (mt: 19) - Wallet and account balances
2. **OrdersSnapshot** (mt: 23) - Open orders
3. **PositionsSnapshot** (mt: 26) - Open positions

The **WalletSnapshot** includes a sequence number (`sn` from `MessageHeader`) that serves as the starting point for sequence tracking. Store this value and use it to validate subsequent heartbeat sequence numbers (see [Heartbeat](#heartbeat-trading)).

### Placing Orders

```typescript
interface OrderRequest {
  mt: 22;
  sn?: number;         // Unique, non-zero — echoed as `cid` on the mt: 3 status
  rq: number;          // Request ID (strictly increasing, API equivalent to client_order_id - enforced by smart contract only for API orders)
  mkt: number;         // Market ID
  acc: number;         // Account ID
  oid?: number;        // Order ID (for modify/cancel)
  t: OrderType;        // Order type
  p?: number;          // Limit price (0 for market)
  s: number;           // Size (scaled)
  a?: string;          // Amount (for collateral increase)
  ms?: number;         // Maximum market order price slippage, bps
  tif?: number;        // Time-in-force block - The last block number on the Monad chain where this order is valid
  fl: OrderFlags;      // Flags (PostOnly, FOK, IOC)
  tp?: number;         // Trigger price (stop/TP orders)
  tpc?: number;        // Trigger condition (1=GTELast, 2=LTELast, 3=GTEMark, 4=LTEMark)
  tr?: number;         // Linked trigger request ID
  lp?: number;         // Linked position ID
  lv: number;          // Leverage (hundredths, e.g., 1000 = 10x)
  lb: number;          // Last execution block: head < lb <= head + market.order_ttl_blocks, or 0
  bf?: number;         // Builder fee, hundred-thousandths (1 = 0.1 bps) — builder-bound keys only
}
```

`bf` is only valid on a **builder-bound** API key, and only up to the ceiling
that key was enrolled with; the builder code itself comes from the key, never
from the request. See [Integrations → Builder codes](./integrations.md#builder-codes).

**Delivery Semantics & Idempotency**:

`rq` (Request ID) is an idempotency key scoped per account. The server guarantees **at-most-once** execution per `rq`.

The Request ID is equivalent to a client order id on non-dex exchanges (applicable only for orders sent via API, not for direct on-chain transactions).

Multiple requests via the API with the same Request ID only results in a single execution.

For smart contract / SDK users placing non API orders the value maybe set to anything to identify the order and is non-unique.

**Request ID generation**:

`rq` must be strictly increasing. The server tracks the last processed ID as `lfr` on the Account object (in WalletSnapshot mt: 19 and AccountUpdate mt: 21).

1. Seed local counter from `account.lfr` on connect to trading websocket
2. For each order: `rq = max(localCounter, account.lfr) + 1`

Submitting `rq <= lfr` fails with `sr: 32` (OrderDescIdTooLow).

**Retries**:

Client side is responsible for retries and should follow the following rules:
1. Retry with original RequestID:
   a. Before receiving any status update for the original request
   b. Before `LastExecBlock` expiration
2. Retry with new RequestID only when:
   a. Failure status received for the original request
   b. Current known block (eg. `Heartbeat.h`) is greater or equal to `LastExecBlock` of the original
   request and no status updates were received - only if all block updates / heartbeats
   after order posting were observed (i.e. there were no reconnections)
For each RequestID, multiple `Order` messages with status updates can be received.
Client side is responsible for deduplication of these messages, processing only:
  a. The first failure message if all received messages are failures (`OrderStatusFailed`)
  b. The first non-failure message received (OrderStatusOpen, OrderStatusPartiallyFilled, OrderStatusFilled, OrderStatusCanceled, OrderStatusUntriggered, OrderStatusTriggered, OrderStatusExecuted)

| Scenario | Action |
|----------|--------|
| No status received yet, `lb` not expired | Retry with **same** `rq` |
| `sr: 32` (OrderDescIdTooLow) | Retry **once** with new `rq` (common with multiple clients/tabs) |
| Head block ≥ `lb`, no status received, no reconnections since posting | Retry with **new** `rq` |

**Client-side deduplication**:

Multiple updates may arrive for a single `rq`:
- First non-failure status (`st: 2–5, 8, 9, 10`) is definitive — ignore everything after, including later failures
- If only failures (`st: 7`) arrive, process the first one only
- After retrying with a new `rq`, ignore late failures from the old `rq`

**Trigger Orders**:

- Set `lb: 0` on trigger orders. A non-zero `lb` is ignored after admission — each execution attempt gets a server-assigned window — but a stale value (`lb <= head`) is still rejected with `last exec block already expired`. Trigger lifetime is governed by `tif`, the trigger condition, and the `tr`/`lp` links.
- `tp` + `tpc`: The order will not be posted until the market last price crosses the trigger price according to the condition (GTE or LTE)
- `tr`: Links this trigger order to another request. When the linked request results in a trade, the trigger activates; when it fails, the trigger is cancelled. If the linked request places an order, this trigger links to that order — activating when it fills, cancelling when it is cancelled.
- `lp`: Links the trigger order to a position. The trigger is cancelled when the position is closed or inverted.

**Order Types**:

| Value | Name |
|-------|------|
| 1 | OpenLong |
| 2 | OpenShort |
| 3 | CloseLong |
| 4 | CloseShort |
| 5 | Cancel |
| 6 | IncreasePositionCollateral |
| 7 | Change |

**Order Flags**:

| Value | Name |
|-------|------|
| 0 | GoodTillCancel |
| 1 | PostOnly |
| 2 | FillOrKill |
| 4 | ImmediateOrCancel |

**Last execution block (`lb`)**:

- `lb: 0` is legal on **any** order type, not just triggers. The server substitutes the market's maximum window
- A non-zero `lb` must be `> head`, on every order type — `0 < lb <= head` is rejected with `last exec block already expired`
- The ceiling is `head + market.order_ttl_blocks` (see the rejection table below for the exact condition)
- The server also **clamps** `lb` down to that ceiling — you cannot buy a longer validity window than the market allows, and a value that passed admission can still be shortened
- `order_ttl_blocks` is per-market and subject to change — read it from `/api/v1/pub/context` or `mt: 8` at runtime, never hardcode it

**Example - Open Long**:
```typescript
let sn = 0;                          // Outbound frame counter, never 0
const ttl = market.order_ttl_blocks; // From /api/v1/pub/context or mt: 8

ws.send(JSON.stringify({
  mt: 22,
  sn: ++sn,                    // Echoed as `cid` on the mt: 3 status for this frame
  rq: Date.now(),              // Unique request ID
  mkt: 1,                      // BTC market (mainnet)
  acc: accountId,              // Your account ID
  t: 1,                        // OpenLong
  p: 95000 * 10,               // Price $95,000 (1 decimal)
  s: 10000,                    // 0.1 BTC (5 decimals)
  fl: 0,                       // GTC
  lv: 1000,                    // 10x leverage
  lb: currentBlock + ttl       // Ceiling: head + order_ttl_blocks
}));
```

**Input Validation** (recommended for production):
- `size > 0` - Reject zero or negative sizes
- `leverage` within market limits - Check `MarketConfig.initial_margin` (e.g., 1000 = 10% = max 10x)
- `marketId` is valid - Verify against `/api/v1/pub/context` markets
- `price > 0` for limit orders, `price = 0` for market (IOC)
- `lb` is `0`, or satisfies `head < lb <= head + market.order_ttl_blocks` where `head` is the latest `Heartbeat.h` (see **Last execution block** above)
- WebSocket is connected - Check `ws.readyState === WebSocket.OPEN`

**Example - Cancel Order**:
```typescript
ws.send(JSON.stringify({
  mt: 22,
  sn: ++sn,
  rq: Date.now(),
  mkt: 1,
  acc: accountId,
  oid: orderIdToCancel,
  t: 5,  // Cancel
  s: 0,
  fl: 0,
  lv: 0,
  lb: currentBlock + ttl
}));
```

### Command Status (mt: 3)

Every `mt: 22` frame receives **exactly one** `StatusResponse` (`mt: 3`) on the
`sid: 100` command-status stream. It reports whether the gateway accepted the
frame — not what happened to the order.

```typescript
interface StatusResponse {
  mt: 3;
  sid: 100;         // Command-status stream
  sn: number;       // Server-assigned; not contiguous per stream
  cid?: number;     // The `sn` you sent — omitted entirely when that `sn` was 0
  status: {
    code: number;   // 0 = accepted for forwarding; 400 = bad request; 403 = read-scoped key
    error: string;
  };
}
```

**`code: 0` means accepted for forwarding — not posted, not filled.** The order's
real outcome arrives later on the `mt: 24` OrdersUpdate stream. A non-zero `code`
means the gateway rejected the frame before it reached the chain, and **no
`mt: 24` message will ever follow for it**.

**Correlation**: `cid` echoes the `sn` from your outbound frame, not `rq`, and is
omitted whenever it would be zero. Set a unique non-zero `sn` on every `mt: 22`
frame — without one, neither acknowledgements nor rejections can be matched to
the order that caused them.

**Rejection reasons** (non-zero `code`):

| `error` | Condition | `code` |
|---|---|---|
| `order already expired` | `tif > 0 && tif <= head` | 400 |
| `last exec block already expired` | `lb > 0 && lb <= head` | 400 |
| `last exec block too high` | `tp == 0 && lb > head + order_ttl_blocks` | 400 |
| `trigger price condition is not specified` | `tp > 0 && tpc == 0` | 400 |
| `order type is not provided` / `invalid order type` | invalid `t` | 400 |
| `builder fee not permitted for this api key` | `bf` above the key's ceiling, or any `bf` on a non-builder key | 400 |
| `api key lacks trade scope` | read-scoped key | 403 |

**Failures that close the connection instead**: an unknown `mkt`, an `acc` not
owned by the connected wallet, and any frame that fails to parse produce no
`mt: 3` at all — the server closes with code `1011` (`failed to process`). Do not
wait on a status that will never arrive; treat an unexpected close as a failure of
every request still in flight. See
[WebSocket Close Codes](./README.md#websocket-close-codes).

`mt: 3` reports admission only; everything after arrives on `mt: 24`. Note
`sr: 14` (`ExceedsLastExecutionBlock`) can be produced without any transaction
reaching the chain — do not treat it as evidence that a transaction was submitted.

### Order Updates (mt: 24)

```typescript
interface WalletOrders {
  mt: 24;
  at: BlockTimestamp;
  d: Order[];
}
```

Orders with `r: true` should be removed from open orders.

### Fill Updates (mt: 25)

```typescript
interface WalletFills {
  mt: 25;
  at: BlockTimestamp;
  d: Fill[];
}
```

### Position Updates (mt: 27)

```typescript
interface WalletPositions {
  mt: 27;
  at: BlockTimestamp;
  d: Position[];
}
```

### Account Updates (mt: 21)

```typescript
interface Account {
  mt: 21;
  in: number;       // Instance ID
  id: number;       // Account ID
  fr: boolean;      // Is frozen
  fw: boolean;      // Allows forwarding
  ft: number;       // Fee tier — indexes the market's maker_fees / taker_fees arrays
  lfr: number;      // Last forwarded request ID (use to seed `rq` generation)
  b: string;        // Balance
  lb: string;       // Locked balance
  h?: AccountEvent[];  // Recent events
}
```

An `mt: 21` update is how a **fee-tier change** reaches you: the tier is
reassigned in the background from trading volume, so re-read `ft` on every
account update rather than caching it from the snapshot. See
[Fees & fee tiers](./README.md#fees--fee-tiers).

### Account Stats (mt: 28)

`AccountStatsUpdate` (mt: 28) carries an `AccountStats` body — per-account
trading statistics (see [AccountStats](./types.md#accountstats)).

```typescript
interface AccountStatsUpdate {
  mt: 28;
  // AccountStats fields (see ./types.md#accountstats)
}
```

Account stats are also delivered in the **WalletSnapshot** (mt: 19) via the
wallet's `sts?` field.

### Heartbeat (Trading) {#heartbeat-trading}

On the trading WebSocket, sequence tracking requires special initialization:

1. Initialize `lastSn` from the `sn` field in the **WalletSnapshot** (mt: 19) received after authentication.
2. Each subsequent heartbeat must have `sn === previousSn + 1`.
3. On sequence gap (missed heartbeat), **force reconnect** — the gap means messages may have been lost.

```typescript
let lastSn: number | undefined;

// On WalletSnapshot (mt: 19)
lastSn = walletMessage.sn;

// On Heartbeat (mt: 100)
if (lastSn != null && heartbeat.sn !== lastSn + 1) {
  // Sequence gap detected — reconnect to get fresh state
  ws.close();
  reconnect();
  return;
}
lastSn = heartbeat.sn;
```

### Keep-Alive

Send periodic pings to keep the trading connection alive:

```typescript
setInterval(() => {
  ws.send(JSON.stringify({
    mt: 1,  // Ping
    t: Date.now()
  }));
}, 30000);
```

Application pings count toward the request budget on both servers. At 30s that is
2/min — negligible against the trading budget, but 20% of the market-data budget.
**Market-data connections do not need `mt: 1` at all**: data arrives every block and
the server keeps the socket alive at the protocol level. Reserve the budget for
subscriptions. See [Rate Limits](./README.md#rate-limits).

### Error Handling

**Close Code 3401**: Authentication failure
Reconnect and re-send a fresh signed `ApiKeySignIn` frame
  (new `timestamp` + `nonce`, re-signed) as the first message

**Close Code 1013**: The client fell behind — its send buffer stayed full for the
whole server I/O timeout and the connection was dropped. Reconnect after a pause and
read frames off the socket into your own queue instead of processing them inline;
resubscribing to the same firehose without that change reproduces it.

See [WebSocket Close Codes](./README.md#websocket-close-codes) for the full list.

```typescript
function handleClose(event) {
  if (event.code === 3401) {
    // Auth failed — just reconnect; the onopen handler re-sends a freshly
    // signed ApiKeySignIn frame (new timestamp + nonce) as the first message.
    reconnect();
  } else {
    // Everything else, 1013 (too slow) included — reconnect with backoff
    // (applied by reconnect()).
    reconnect();
  }
}
```

### Reconnection Strategy

```typescript
const RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 32000, 60000];
let retryCount = 0;
let ws: WebSocket;

// Open the socket and wire up the handlers. Called again by reconnect().
function connect() {
  ws = new WebSocket(`${WS_URL}/ws/v1/trading`);

  ws.onopen = async () => {
    // Authenticate immediately: the first frame is a signed ApiKeySignIn (mt: 29).
    const timestamp = Date.now().toString();
    const nonce = randomBytes(16).toString('base64url');
    const canonical = [CHAIN_ID, 'trading-ws-signin', timestamp, nonce].join('\n');
    const sig = await ed.signAsync(Buffer.from(canonical), privateKey);

    ws.send(JSON.stringify({
      mt: 29,             // MsgTypeApiKeySignIn
      chain_id: CHAIN_ID,
      api_key: API_KEY,   // X-API-Key token from enrollment
      timestamp,
      nonce,
      signature: Buffer.from(sig).toString('base64url'),
    }));

    onConnectSuccess();
  };

  ws.onmessage = (event) => { /* handle snapshots/updates */ };
  ws.onclose = handleClose;  // the onclose handler shown under "Error Handling"
}

function reconnect() {
  const delay = RETRY_DELAYS[Math.min(retryCount, RETRY_DELAYS.length - 1)];
  setTimeout(() => {
    retryCount++;
    connect();
  }, delay);
}

function onConnectSuccess() {
  retryCount = 0;
}

connect();
```

---

## Sequence Numbers

- `heartbeat` and `gas-stats` streams have continuous sequence numbers
- Other streams may have gaps (e.g., when no activity)
- Track `sn` to detect missed messages
- On gap detection, resubscribe to get fresh snapshot
