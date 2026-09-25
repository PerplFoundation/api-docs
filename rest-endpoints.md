# REST API Endpoints

Base URL: `${PERPL_API_URL}` (default: `https://app.perpl.xyz/api`)

Authenticated endpoints are signed per request with an API key (`X-API-*`
headers) — see [Authentication](./authentication.md). To obtain a key, see
[Integrations](./integrations.md).

## Public Endpoints

### GET /api/v1/pub/context

Returns global protocol configuration including chain, markets, and tokens.

**Authentication**: Optional. Unauthenticated requests return the public context; providing an API-key signature personalizes the response.

**Response**:
```typescript
interface Context {
  chain: Chain;
  instances: ProtocolInstance[];
  tokens: Token[];
  markets: Market[];
}
```

Each market's `config` carries the full per-tier fee schedule
(`maker_fees` / `taker_fees`) — see [Fees & fee tiers](./README.md#fees--fee-tiers).

**Example**:
```bash
# Using default live URL
curl https://app.perpl.xyz/api/v1/pub/context

# Or using environment variable
curl ${PERPL_API_URL:-https://app.perpl.xyz/api}/v1/pub/context
```

---

### GET /api/v1/market-data/:market_id/candles/:resolution/:from-:to

Returns OHLCV candlestick data.

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| market_id | number | Market ID (e.g., 1 for BTC on mainnet) |
| resolution | number | Candle resolution in seconds |
| from | number | Start timestamp (ms) |
| to | number | End timestamp (ms) |

**Limits**:
- Maximum **1024 candles** per request

**Supported Resolutions** (seconds):
- `60` (1m)
- `300` (5m)
- `900` (15m)
- `1800` (30m)
- `3600` (1h)
- `7200` (2h)
- `14400` (4h)
- `28800` (8h)
- `43200` (12h)
- `86400` (1d)

**Response**:
```typescript
interface CandleSeries {
  mt: number;           // Message type
  at: BlockTimestamp;   // Timestamp
  r: number;            // Resolution (seconds)
  d: Candle[];          // Candle data
}

interface Candle {
  t: number;    // Open timestamp (ms)
  o: number;    // Open price (scaled)
  c: number;    // Close price (scaled)
  h: number;    // High price (scaled)
  l: number;    // Low price (scaled)
  v: string;    // Volume (collateral token)
  n: number;    // Number of trades
}
```

**Example**:
```bash
# Get 1-hour BTC candles for last 24 hours
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
FROM=$(($(date +%s) * 1000 - 86400000))
TO=$(($(date +%s) * 1000))
curl "${API_URL}/v1/market-data/1/candles/3600/${FROM}-${TO}"
```

A market with no candles yet returns an empty `d`.

---

### GET /api/v1/market-data/:market_id/funding/:from-:to

Returns the funding events of a market, oldest first — one per funding interval the
market had a rate set for. An interval whose rate was never set is absent from the
series.

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| market_id | number | Market ID (e.g., 1 for BTC on mainnet) |
| from | number | Start timestamp (ms), inclusive |
| to | number | End timestamp (ms), inclusive |

`from` and `to` are matched against the timestamp each event **applies** at (`at.t`
of the event, see below), not the timestamp it was published at.

**Limits**:
- The period may cover at most **1024 funding intervals** of the market. The interval
  is `funding_interval_sec` of the market configuration (`GET /v1/pub/context`), so
  longer history is retrieved in several requests.

**Response**:
```typescript
interface FundingSeries {
  mt: number;           // Message type
  at: BlockTimestamp;   // Block/timestamp of the most recent event in the series
  m: number;            // Market ID
  d: FundingEvent[];    // Funding events, oldest first
}
```

`FundingEvent` is the same type the `funding@<chain_id>` WebSocket stream and
`GET /v1/pub/context` report — see **[Types](./types.md#fundingevent)**.

Two behaviours to plan for:

- **At least one event** is returned whenever the market has any funding history, even
  if the requested period contains none of its own: a period shorter than the funding
  interval resolves to the rate that was in force over it. A market with no funding
  history at all returns an empty `d`.
- The **most recent event** may carry an estimated `at.t` (see below), which is
  corrected within about a minute.

**Example**:
```bash
# Get BTC funding history for the last 24 hours
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
FROM=$(($(date +%s) * 1000 - 86400000))
TO=$(($(date +%s) * 1000))
curl "${API_URL}/v1/market-data/1/funding/${FROM}-${TO}"
```

For every market in one request, see below.

---

### GET /api/v1/market-data/funding/:from-:to

Returns the funding events of **all markets** applied within the requested period,
keyed by market ID — the same data the per-market endpoint above serves, for the
whole exchange in one request.

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| from | number | Start timestamp (ms), inclusive |
| to | number | End timestamp (ms), inclusive |

`from` and `to` are matched against the timestamp each event **applies** at, exactly
as for a single market.

**Limits**:
- The period may cover at most **128 funding intervals of the market with the
  shortest interval** — an order of magnitude below the single-market cap, because
  the response carries that many events for every market at once. Markets may be configured with
  different `funding_interval_sec`, and the cap is applied so that no single market
  can overrun it, so longer history is retrieved in several requests stepping by the
  shortest interval of the markets in `GET /v1/pub/context`. Deep history of one
  market is cheaper to page over the per-market endpoint, which allows 1024.
- `to` may run up to the **longest** funding interval past the current time, so the
  most recent event of every market is reachable — a funding event is timestamped
  ahead of the chain, as above.

**Response**:
```typescript
interface MarketFundingSeries {
  mt: number;           // Message type
  at: BlockTimestamp;   // Block/timestamp of the most recent event across all markets
  d: { [market_id: number]: FundingEvent[] };  // Events per market, oldest first
}
```

`FundingEvent` is the same type the per-market endpoint, the `funding@<chain_id>`
WebSocket stream and `GET /v1/pub/context` report — see
**[Types](./types.md#fundingevent)**.

Behaviours to plan for, on top of the per-market ones above:

- Each market's series is resolved **against its own history**, so a period shorter
  than a market's funding interval still resolves to the rate that was in force over
  it. One request can therefore return a different number of events per market.
- A market with **no funding history at all is absent** from `d` rather than present
  with an empty array, as are markets not listed in `GET /v1/pub/context`. Do not
  assume a key exists for every market.

**Example**:
```bash
# Get the funding history of every market for the last 24 hours
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
FROM=$(($(date +%s) * 1000 - 86400000))
TO=$(($(date +%s) * 1000))
curl "${API_URL}/v1/market-data/funding/${FROM}-${TO}"
```

---

### GET /api/v1/market-data/:market_id/book

Returns the current L2 order book of a market — the price and size of every resting
level, each side ordered away from the spread.

This is the snapshot the `order-book@<market_id>` WebSocket stream opens with, served
over HTTP for a client that wants the book once rather than following it. A client
tracking the book continuously should subscribe to the stream instead — the deep tail
of the book is not served here at all (see **Limits**).

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| market_id | number | Market ID (e.g., 1 for BTC on mainnet) |

**Query Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| levels | number | 100 | Price levels per side, counted from the spread outwards |

**Limits**:
- `levels` must be an integer **1–100**. A value outside that range is **rejected
  with 400, not clamped** — a client asking for depth this endpoint does not serve is
  told so rather than quietly served less and left to assume the book ends there.

**Response**:
```typescript
interface L2Book {
  mt: 15;               // Message type (L2BookSnapshot)
  sn: number;           // Block the book is current as of
  at: BlockTimestamp;   // Block/timestamp of the snapshot
  bid: L2PriceLevel[];  // Bid levels, ordered away from the spread (best first)
  ask: L2PriceLevel[];  // Ask levels, ordered away from the spread (best first)
}

interface L2PriceLevel {
  p: number;  // Price (scaled by market price_decimals)
  s: number;  // Size (scaled by market size_decimals)
  o: number;  // Number of orders at this level
}
```

Unlike the WebSocket stream, this endpoint only ever answers a snapshot — there is no
`mt: 16` update form and no `o: 0` level-removal convention to handle. A side with
nothing resting is an empty array, never `null`.

Note the sequencing difference from the history endpoints: `sn` is the block the
**whole book** is current as of, rather than being derived from the last entry of a
series.

**Status codes with special meaning**:
| Code | Meaning |
|------|---------|
| 400 | Unknown market, or `levels` outside 1–100 |
| 503 | The book of this market is not available yet — the case for a short while after the service starts. Retry |

**Example**:
```bash
# Top 10 levels of each side of the BTC book
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
curl "${API_URL}/v1/market-data/1/book?levels=10"
```

---

### GET /api/v1/market-data/:market_id/ticker

Returns the current state of a market: its oracle, mark, last and mid price, its best
bid and ask, and its daily volume, open interest and total value locked.

This is the state the `market-state@<chain_id>` WebSocket stream publishes, served
over HTTP for a client that wants it once rather than following it — and **keyed by
market ID the way the stream keys it**, so a single-market response is a map with one
entry, not a bare object.

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| market_id | number | Market ID (e.g., 1 for BTC on mainnet) |

**Response**:
```typescript
interface MarketStateUpdate {
  mt: 9;                                   // Message type (MarketStateUpdate)
  sn: number;                              // Newest block any state in `d` is stamped with
  d: { [market_id: number]: MarketState };
}
```

`MarketState` is the same type the WebSocket stream reports — see
**[Types](./types.md#marketstate)**. Each market's state carries the block and
timestamp **it** is current as of in its own `at`, so `d` may mix blocks; the
top-level `sn` is the newest of them.

**Status codes with special meaning**:
| Code | Meaning |
|------|---------|
| 400 | Unknown market |
| 503 | The state of this market is not available yet — the case for a short while after the service starts. Retry |

**Example**:
```bash
# Current state of the BTC market
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
curl "${API_URL}/v1/market-data/1/ticker"
```

For every market in one request, see below.

---

### GET /api/v1/market-data/ticker

Returns the current state of **all markets**, keyed by market ID — the same data the
per-market endpoint above serves, for the whole exchange in one request.

**Authentication**: None

**Response**: `MarketStateUpdate`, exactly as above.

Behaviours to plan for:

- A market whose state **has not been received yet is absent** from `d` rather than
  present with zeroed values, as are markets hidden from the API. Do not assume a key
  exists for every market in `GET /v1/pub/context`.
- An empty result is not served: if **no** market state is available yet the request
  is refused with 503 instead, so a caller retries rather than renders a market-less
  exchange.

**Status codes with special meaning**:
| Code | Meaning |
|------|---------|
| 503 | No market state is available yet — the case for a short while after the service starts. Retry |

**Example**:
```bash
# Current state of every market
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}
curl "${API_URL}/v1/market-data/ticker"
```

---

## API Keys

API keys are the **primary programmatic authentication** mechanism: an Ed25519 key pair enrolled once via a wallet signature, after which every request is signed with the private key (headers `X-API-Key`, `X-API-Timestamp`, `X-API-Nonce`, `X-API-Signature`).

- Enrolling a key (`POST /api/v1/api-key/payload` + `POST /api/v1/api-key/enroll`) — see **[Integrations](./integrations.md)**.
- Signing each request / the canonical string format — see **[Authentication](./authentication.md)**.
- Listing and revoking keys is handled by the web UI (`/apikeys`), not the API.

---

## Profile Endpoints

### GET /api/v1/profile/ref-code

Get your current referral code.

**Authentication**: API-key signature

**Response**:
```typescript
interface RefCode {
  code: string;
  limit?: number;      // Max profiles that can be created with this code
  used?: number;       // Profiles already created with this code
  volume?: Amount;     // Total volume generated by referred profiles (T1 only, all time), CNS
  created_at: number;  // Ref code creation timestamp (ms)
}
```

Returns 404 with empty code if no referral code assigned.

---

### GET /api/v1/profile/announcements

Get announcements.

**Authentication**: Optional. Works unauthenticated (public audience); an API-key signature personalizes the returned announcements.

**Response**:
```typescript
interface AnnouncementsResponse {
  ver: number;
  active: Announcement[];
}

interface Announcement {
  id: number;
  title: string;
  content: string;
}
```

---

## Trading State Endpoints

The open orders, open positions and wallet of the calling wallet, as of the block the
trading state is current at. Each is the snapshot the corresponding WebSocket stream
opens with, served over HTTP for a client that wants it once rather than following it
— the shapes are identical, so a client that already parses the stream needs no new
types.

These are **live state, not history**: the set of open orders and the value of a
position are recomputed as blocks arrive, and are not reconstructible by paging the
[history endpoints](#trading-history-endpoints) without replaying every event.

All three are signed with an API key (`X-API-*` headers — see
[Authentication](./authentication.md)). A read-only key is sufficient.

Common to all three:

| Code | Meaning |
|------|---------|
| 404 | The calling wallet holds no exchange account. See [Creating an Exchange Account](./README.md#creating-an-exchange-account) |

`sn` is the block the **whole snapshot** is current as of, unlike the history
endpoints where it comes from the last entry of the page.

### GET /api/v1/trading/orders

Returns the wallet's open orders across every account and market it holds one in: the
orders still resting on the book, together with the trigger orders still waiting on
their condition.

**Authentication**: API-key signature (any scope)

**Response**:
```typescript
interface WalletOrders {
  mt: 23;              // Message type (OrdersSnapshot)
  sn: number;          // Block the snapshot is current as of
  at: BlockTimestamp;  // Block/timestamp of the snapshot
  d: Order[];          // Open orders, older to newer
}
```

See [Types](./types.md#order) for the `Order` structure. A wallet with nothing open
is served an empty `d`.

**Example**:
```bash
# signed with X-API-* headers, see authentication.md
curl "${PERPL_API_URL:-https://app.perpl.xyz/api}/v1/trading/orders"
```

---

### GET /api/v1/trading/positions

Returns the wallet's open positions across every account and market it holds one in.

**Authentication**: API-key signature (any scope)

**Response**:
```typescript
interface WalletPositions {
  mt: 26;              // Message type (PositionsSnapshot)
  sn: number;          // Block the snapshot is current as of
  at: BlockTimestamp;  // Block/timestamp of the snapshot
  d: Position[];       // Open positions, older to newer
}
```

See [Types](./types.md#position) for the `Position` structure — in particular `fee`
and `cfee`, whose meaning differs between a snapshot and an event. A wallet with
nothing open is served an empty `d`.

---

### GET /api/v1/trading/wallet

Returns the calling wallet: its nonce and fee level, the exchange accounts it owns,
and the all-time statistics of each.

**Authentication**: API-key signature (any scope)

**Response**:
```typescript
interface Wallet {
  mt: 19;              // Message type (WalletSnapshot)
  sn: number;          // Block the snapshot is current as of
  at: BlockTimestamp;  // Block/timestamp of the snapshot
  addr: string;        // Wallet address
  n: number;           // Current nonce
  fl: number;          // Fee level of the wallet
  as: Account[];       // Exchange accounts
  sts: AccountStats[]; // All-time statistics, one per account
}
```

See [Types](./types.md#account) and [Types](./types.md#accountstats) for the element
structures.

`sts` carries the **all-time** statistics of each account, exactly as the WebSocket
snapshot does. `as[].fw` is the order-forwarding flag — it must be
true before any order from this account is accepted, see
[Enabling Order Forwarding](./README.md#enabling-order-forwarding-one-click-trading).

---

## Order Submission

### POST /api/v1/trading/orders

Places, changes or cancels orders over HTTP, for a client whose flow does not justify
holding a WebSocket connection open. An order submitted here is validated, forwarded
and settled exactly as the same order sent over the WebSocket.

**Authentication**: API-key signature with the **trade** scope. A read-only key is
refused with 403 before the body is read.

> **Prerequisite**: order forwarding — "One-Click Trading" — must be enabled for the
> account (`Account.fw == true`), exactly as for WebSocket orders. See
> [Enabling Order Forwarding](./README.md#enabling-order-forwarding-one-click-trading).

**Request**:
```typescript
interface BatchOrderRequest {
  mt?: 30;          // Optional; the endpoint does not require a message header
  sn?: number;      // Optional; echoed as `cid` on the response
  d: OrderSpec[];   // 1–100 orders, in the order they should reach the exchange
}
```

`OrderSpec` is the payload of a WebSocket `OrderRequest` (mt: 22) **without** the
message header — the same fields with the same meanings, listed in
[Types](./types.md#orderspec) and documented at
[Placing Orders](./websocket.md#placing-orders). The orders may name different markets
and different accounts of the calling wallet.

**Why a batch**: this is the only transport where several orders can be submitted at
one round trip of latency. A WebSocket client already has the connection open and
pipelines frames down it; an HTTP client would otherwise pay a round trip per order
and could not tell in which sequence its concurrent requests were handled. Sent as one
request, they are forwarded in the order they are listed — so a batch that cancels one
order and places another is applied in that sequence. One order is a batch of one.

**Response**:
```typescript
interface BatchStatusResponse {
  mt: 31;             // Message type (BatchStatusResponse)
  cid?: number;       // `sn` of the request, when it carried one
  status: Status;     // Status of the request as a whole — zero code when the orders were judged
  statuses?: Status[];// One status per order, at the position of the order it answers
}

interface Status {
  code: number;       // 0 = accepted for forwarding
  error?: string;     // Human-readable description
}
```

#### Reading the response

Four rules, none of which the HTTP status alone tells you:

1. **HTTP 200 means "judged", not "accepted".** The request is answered 200 whenever
   the orders were looked at individually, however many of them were refused. Read
   every element of `statuses`, not the HTTP status.
2. **A batch is not a transaction.** Orders are judged one by one and orders that were
   accepted stay accepted when a later one is refused — there is nothing to roll back
   an order the exchange has already been told about.
3. **Orders are identified by position.** The i-th element of `statuses` answers the
   i-th element of the request's `d`. There is no echo of the order in the status.
4. **A zero code is an acknowledgement of forwarding, not an outcome.** Whether the
   order posted, filled or was rejected by the exchange is settled a block later, and
   read from the WebSocket order updates (mt: 24) or a `GET` of this endpoint.

A request refused **as a whole** — unreadable, empty, longer than 100 orders, from a
key without the trade scope, or while the gateway does not know the current block yet
— never reaches the orders. It carries no per-order statuses: the reason is reported
as the top-level `status` and its code is repeated as the HTTP status.

#### Rate limiting

**Every order costs one unit of the caller's request-rate allowance**, being the work
one request would have carried. Batching saves round trips; it does not raise the rate
at which orders may be submitted.

The allowance is charged as the batch is processed, so a batch that outruns it is
served up to that point and refused from there on. Orders past the allowance are
answered `429` **in their own positions** and may be retried — the request as a whole
is still 200. A batch of 100 is sized so a caller with a full allowance can pay for it
in full.

#### Builder fees

Builder attribution comes from the **authenticated API key**, never the request body:
the builder code is taken from the key's signed enrolment payload and cannot be named
in an order. A `bf` above the key's enrolled ceiling — or any `bf` on a key with no
builder binding — is refused with 400 rather than silently clamped, so an integrator's
own accounting can never disagree with the chain. See
[Builder codes](./integrations.md#builder-codes).

#### Status codes

Per-order, in `statuses[i].code`:

| Code | Meaning |
|------|---------|
| 0 | Accepted for forwarding to the exchange |
| 400 | Unknown market; order rejected by validation; `bf` not permitted for this key; request ID below the last accepted for the account (a new order's must be strictly greater than `lfr`, see below) |
| 403 | The order names an account the calling wallet does not own |
| 429 | The caller's request-rate allowance ran out partway through the batch. Retryable |
| 503 | Order submission is backed up. Retryable |

Batch-level, in `status.code`, and repeated as the HTTP status:

| Code | Meaning |
|------|---------|
| 400 | Malformed body, no orders, or more than 100 orders |
| 403 | The API key lacks the `trade` scope |
| 503 | The gateway does not know the current exchange block yet — the case for a short while after the service starts. Retry |

Request IDs (`rq`) are tracked per account. A **new** order's `rq` must be **strictly
greater** than `lfr`, the last request ID forwarded for that account: `rq <= lfr` is
rejected as `OrderDescIdTooLow`. Seed a counter from `lfr` on the wallet snapshot and
take `rq = max(counter, lfr) + 1`. An id below the last one accepted here is refused
with 400 up front, rather than a block later on a stream this client may not be
reading. Re-sending the **same** `rq` is the retry mechanism and stays accepted — see
[Placing Orders](./websocket.md#placing-orders) for the at-most-once semantics.

#### The last execution block over HTTP

`lb` (last execution block) follows the same rule as over WebSocket —
`head < lb <= head + market.order_ttl_blocks`, or `0` — but an HTTP client has no
heartbeat stream to read `head` from. Take it from `sn` on any
[Trading State](#trading-state-endpoints) response, or from `sn` on the
[ticker](#get-apiv1market-dataticker) — every one of them is stamped with the block
it is current as of. An order whose `lb` has already passed is refused with 400
(`last exec block already expired`); one beyond the market's TTL is refused with 400
(`last exec block too high`).

**Example** — cancel one order and place another, applied in that sequence:
```bash
# signed with X-API-* headers, see authentication.md
curl -X POST "${PERPL_API_URL:-https://app.perpl.xyz/api}/v1/trading/orders" \
  -H 'Content-Type: application/json' \
  -d '{
    "d": [
      { "rq": 1001, "mkt": 1, "acc": 7, "oid": 4242, "t": 5, "fl": 0, "lv": 0, "lb": 0 },
      { "rq": 1002, "mkt": 1, "acc": 7, "t": 1, "p": 6500000, "s": 100000,
        "fl": 1, "lv": 500, "lb": 12045890 }
    ]
  }'
```

A partially refused batch — the cancel went through, the placement did not:
```json
{
  "mt": 31,
  "status": { "code": 0 },
  "statuses": [
    { "code": 0 },
    { "code": 403, "error": "invalid account" }
  ]
}
```

---

## Trading History Endpoints

All trading history endpoints are signed with an API key (`X-API-*` headers — see [Authentication](./authentication.md)) and support pagination.

### GET /api/v1/trading/account-history

**Authentication**: API-key signature

**Common Query Parameters**:
| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| page | string | - | Cursor for pagination (from previous response `np`) |
| count | number | 50 | Items per page (max: 100) |

*Note: Server-side filtering by market ID or date range is not currently supported. Filter results client-side if needed.*

**Common Response Pattern**:
```typescript
interface HistoryPage<T> {
  d: T[];      // Data array (newest to oldest)
  np: string;  // Next page cursor
}
```

Get account events (deposits, withdrawals, settlements, etc.)

**Response**:
```typescript
interface AccountHistoryPage {
  d: AccountEvent[];
  np: string;
}

interface AccountEvent {
  at: BlockTxLogTimestamp;  // Timestamp
  in: number;               // Instance ID
  id: number;               // Account ID
  et: AccountEventType;     // Event type
  m?: number;               // Market ID
  r?: number;               // Request ID
  o?: number;               // Order ID
  p?: number;               // Position ID
  a: string;                // Amount change
  b: string;                // Updated balance
  lb: string;               // Locked balance
  f: string;                // Fee (gross: protocol fee + `bfa`)
  bfa?: string;             // Builder-fee portion of `f`, omitted when zero
}
```

**Account Event Types**:
| Value | Name |
|-------|------|
| 0 | Unspecified |
| 1 | Deposit |
| 2 | Withdrawal |
| 3 | IncreasePositionCollateral |
| 4 | Settlement |
| 5 | Liquidation |
| 6 | TransferToProtocol |
| 7 | TransferFromProtocol |
| 8 | Funding |
| 9 | Deleveraging |
| 10 | Unwinding |
| 11 | PositionCollateralDecreased |
| 12 | LastForwardedDescIdReset |

---

### GET /api/v1/trading/fills

Get order fill history.

**Authentication**: API-key signature

**Response**:
```typescript
interface FillHistoryPage {
  d: Fill[];
  np: string;
}

interface Fill {
  at: BlockTxLogTimestamp;
  mkt: number;      // Market ID
  acc: number;      // Account ID
  oid: number;      // Order ID
  t: OrderType;     // Order type
  l: LiquiditySide; // Maker=1, Taker=2
  p?: number;       // Fill price (scaled)
  s: number;        // Filled size (scaled)
  f: string;        // Fee/rebate (gross: protocol fee + `bfa`)
  bfa?: string;     // Builder-fee portion of `f`, omitted when zero
}
```

The rate behind `f` is the market's maker or taker fee at the account's fee tier
— see [Fees & fee tiers](./README.md#fees--fee-tiers).

---

### GET /api/v1/trading/order-history

Get historical order events.

**Authentication**: API-key signature

**Response**:
```typescript
interface OrderHistoryPage {
  d: Order[];
  np: string;
}
```

See [Types](./types.md#order) for Order structure.

---

### GET /api/v1/trading/position-history

Get position history.

**Authentication**: API-key signature

**Response**:
```typescript
interface PositionHistoryPage {
  d: Position[];
  np: string;
}
```

See [Types](./types.md#position) for Position structure.

---

## Pagination Example

Requests are signed with the API-key headers. `signedRequest(method, target, body)` is
the helper defined in [Authentication](./authentication.md#authenticating-rest-requests) —
note the `request-target` (path + query string) must be signed exactly as sent.

```typescript
async function fetchAllFills() {
  const fills: Fill[] = [];
  let page: string | undefined;

  do {
    const params = new URLSearchParams({ count: '100' });
    if (page) params.set('page', page);
    const target = `/v1/trading/fills?${params.toString()}`;

    // signed with X-API-* headers, see authentication.md
    const response = await signedRequest('GET', target);

    const data: FillHistoryPage = await response.json();
    fills.push(...data.d);
    page = data.np;
  } while (page);

  return fills;
}
```

---

## Portfolio

### GET /api/v1/trading/portfolio/:kind/:period

Returns the calling wallet's equity or PnL as a time series — the chart data behind a
portfolio view, aggregated server-side over a chosen period so a client does not have
to rebuild the curve from
[account history](#get-apiv1tradingaccount-history).

**Authentication**: API-key signature (any scope)

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| kind | string | `equity` or `pnl` |
| period | string | `all`, `day`, `2weeks`, `week` or `month` |

Both are path segments, not query parameters, and both are required — there is no
default for either. A value outside the sets above is a 400.

| Code | Meaning |
|------|---------|
| 400 | `kind` or `period` is not one of the values above |
| 404 | The calling wallet holds no exchange account. See [Creating an Exchange Account](./README.md#creating-an-exchange-account) |

**Response**:
```typescript
interface Portfolio {
  at: BlockTimestamp;   // Block/timestamp of the last update
  chart: ChartPoint[];  // Chart points, older to newer
}
```

See [Types](./types.md#portfolio) for the `ChartPoint` structure. The point spacing is
chosen by the exchange for the requested period. A wallet with no history over the
period is served an empty `chart`.

**Example**:
```bash
API_URL=${PERPL_API_URL:-https://app.perpl.xyz/api}

# All-time equity curve
# signed with X-API-* headers, see authentication.md
curl "${API_URL}/v1/trading/portfolio/equity/all"

# PnL over the last week
curl "${API_URL}/v1/trading/portfolio/pnl/week"
```
