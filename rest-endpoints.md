# REST API Endpoints

Base URL: `${PERPL_API_URL}` (default: `https://testnet.perpl.xyz/api`)

## Public Endpoints

### GET /api/v1/pub/context

Returns global protocol configuration including chain, markets, and tokens.

**Authentication**: None

**Response**:
```typescript
interface Context {
  chain: Chain;
  instances: ProtocolInstance[];
  tokens: Token[];
  markets: Market[];
  features?: Record<string, string>;
}
```

**Example**:
```bash
# Using default testnet URL
curl https://testnet.perpl.xyz/api/v1/pub/context

# Or using environment variable
curl ${PERPL_API_URL:-https://testnet.perpl.xyz/api}/v1/pub/context
```

---

### GET /api/v1/market-data/:market_id/candles/:resolution/:from-:to

Returns OHLCV candlestick data.

**Authentication**: None

**URL Parameters**:
| Parameter | Type | Description |
|-----------|------|-------------|
| market_id | number | Market ID (e.g., 16 for BTC) |
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
API_URL=${PERPL_API_URL:-https://testnet.perpl.xyz/api}
FROM=$(($(date +%s) * 1000 - 86400000))
TO=$(($(date +%s) * 1000))
curl "${API_URL}/v1/market-data/16/candles/3600/${FROM}-${TO}"
```

---

## Authentication Endpoints

### POST /api/v1/auth/payload

Request signing payload for wallet authentication.

**Authentication**: None

**Request**:
```typescript
interface AuthPayloadRequest {
  chain_id: number;  // 10143
  address: string;   // Wallet address
}
```

**Response**:
```typescript
interface AuthPayloadResponse {
  message: string;     // SIWE message to sign
  nonce: string;
  issued_at: number;   // Timestamp (ms)
  mac: string;
}
```

---

### POST /api/v1/auth/connect

Submit signed payload to authenticate.

**Authentication**: None

**Request**:
```typescript
interface AuthConnectRequest {
  chain_id: number;
  address: string;
  message: string;
  nonce: string;
  issued_at: number;
  mac: string;
  signature: string;
  ref_code?: string;
}
```

**Response**:
```typescript
interface AuthConnectResponse {
  nonce: string;
}
```

**Status Codes**:
| Code | Meaning |
|------|---------|
| 200 | Success |
| 418 | Access code required |
| 423 | Access code invalid/exhausted |
| 403 | Access denied |

---

## Profile Endpoints

All profile endpoints require authentication:
- JWT cookie (set by `/api/v1/auth/connect`)
- `X-Auth-Nonce` header with nonce from auth response

### GET /api/v1/profile/ref-code

Get your referral code.

**Response**:
```typescript
interface RefCode {
  code: string;
  limit: number;   // Max uses
  used: number;    // Times used
}
```

Returns 404 with empty code if no referral code assigned.

---

### GET /api/v1/profile/contact-info

Get stored contact info.

**Response**:
```typescript
interface ContactInfo {
  contact: string;
  x_challenge: string;
}
```

---

### POST /api/v1/profile/contact-info

Update contact info.

**Request**:
```typescript
interface ContactInfo {
  contact: string;
  x_challenge: string;
}
```

**Response**: 204 No Content or JSON with updated info

---

### GET /api/v1/profile/announcements

Get announcements. Works with or without authentication.

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

## Trading History Endpoints

### GET /api/v1/trading/account-history

All trading history endpoints require authentication and support pagination.

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
  f: string;                // Fee
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

---

### GET /api/v1/trading/fills

Get order fill history.

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
  f: string;        // Fee/rebate
}
```

---

### GET /api/v1/trading/order-history

Get historical order events.

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

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';

async function fetchAllFills(authNonce: string) {
  const fills: Fill[] = [];
  let page: string | undefined;

  do {
    const url = new URL(`${API_URL}/v1/trading/fills`);
    if (page) url.searchParams.set('page', page);
    url.searchParams.set('count', '100');

    const response = await fetch(url.toString(), {
      headers: { 'X-Auth-Nonce': authNonce },
      credentials: 'include'
    });

    const data: FillHistoryPage = await response.json();
    fills.push(...data.d);
    page = data.np;
  } while (page);

  return fills;
}
```
