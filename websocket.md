# WebSocket API

Perpl provides two WebSocket endpoints for real-time data.

## Endpoints

| Endpoint | Purpose | Authentication |
|----------|---------|----------------|
| `/ws/v1/market-data` | Public market data | None |
| `/ws/v1/trading` | Trading & account data | Required |

**URLs**:
- Market Data: `wss://testnet.perpl.xyz/ws/v1/market-data`
- Trading: `wss://testnet.perpl.xyz/ws/v1/trading`

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
| 4 | AuthSignIn | Client → Server |
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
| 100 | Heartbeat | Server → Client |

---

## Market Data WebSocket

### Connecting

```typescript
const ws = new WebSocket('wss://testnet.perpl.xyz/ws/v1/market-data');
```

### Available Streams

| Stream | Format | Description |
|--------|--------|-------------|
| heartbeat | `heartbeat@<chain_id>` | Block sync heartbeat |
| gas-stats | `gas-stats@<chain_id>` | Gas price updates |
| market-config | `market-config@<chain_id>` | Market configuration |
| market-state | `market-state@<chain_id>` | Prices, volume, OI |
| funding | `funding@<chain_id>` | Funding rate updates |
| candles | `candles@<market_id>*<resolution>` | OHLCV candles |
| order-book | `order-book@<market_id>` | L2 order book |
| trades | `trades@<market_id>` | Recent trades |

**Chain ID**: 10143 (Monad Testnet)

**Candle Resolutions** (seconds): 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200, 86400

### Subscribing

```typescript
// Subscribe to streams
ws.send(JSON.stringify({
  mt: 5,  // SubscriptionRequest
  subs: [
    { stream: 'heartbeat@10143', subscribe: true },
    { stream: 'order-book@16', subscribe: true },    // BTC order book
    { stream: 'trades@16', subscribe: true },        // BTC trades
    { stream: 'candles@16*3600', subscribe: true }   // BTC 1h candles
  ]
}));
```

### Subscription Response

```typescript
interface SubscriptionResponse {
  mt: 6;
  subs: Array<{
    stream: string;
    sid?: number;      // Subscription ID (use to match updates)
    status?: {
      code: number;    // 0 = success
      error?: string;
    };
  }>;
}
```

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
  h: number;  // Latest head block number
}
```

---

## Trading WebSocket

### Connecting & Authenticating

```typescript
const ws = new WebSocket('wss://testnet.perpl.xyz/ws/v1/trading');

ws.onopen = () => {
  // Must authenticate immediately
  ws.send(JSON.stringify({
    mt: 4,  // AuthSignIn
    chain_id: 10143,
    nonce: authNonce,  // From /api/v1/auth/connect
    ses: crypto.randomUUID()
  }));
};
```

### Initial Snapshots

After authentication, you receive snapshots:

1. **WalletSnapshot** (mt: 19) - Wallet and account balances
2. **OrdersSnapshot** (mt: 23) - Open orders
3. **PositionsSnapshot** (mt: 26) - Open positions

### Placing Orders

```typescript
interface OrderRequest {
  mt: 22;
  rq: number;          // Request ID (strictly increasing)
  mkt: number;         // Market ID
  acc: number;         // Account ID
  oid?: number;        // Order ID (for modify/cancel)
  t: OrderType;        // Order type
  p?: number;          // Limit price (0 for market)
  s: number;           // Size (scaled)
  a?: string;          // Amount (for collateral increase)
  tif?: number;        // Time-in-force block
  fl: OrderFlags;      // Flags (PostOnly, FOK, IOC)
  tp?: number;         // Trigger price (stop/TP orders)
  tpc?: number;        // Trigger condition (1=GTE, 2=LTE)
  tr?: number;         // Linked trigger request
  lp?: number;         // Linked position ID
  lv: number;          // Leverage (hundredths, e.g., 1000 = 10x)
  lb: number;          // Last execution block
}
```

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

**Example - Open Long**:
```typescript
ws.send(JSON.stringify({
  mt: 22,
  rq: Date.now(),              // Unique request ID
  mkt: 16,                     // BTC market
  acc: accountId,              // Your account ID
  t: 1,                        // OpenLong
  p: 95000 * 10,               // Price $95,000 (1 decimal)
  s: 10000,                    // 0.1 BTC (5 decimals)
  fl: 0,                       // GTC
  lv: 1000,                    // 10x leverage
  lb: currentBlock + 100       // Valid for 100 blocks
}));
```

**Input Validation** (recommended for production):
- `size > 0` - Reject zero or negative sizes
- `leverage` within market limits - Check `MarketConfig.initial_margin` (e.g., 1000 = 10% = max 10x)
- `marketId` is valid - Verify against `/api/v1/pub/context` markets
- `price > 0` for limit orders, `price = 0` for market (IOC)
- WebSocket is connected - Check `ws.readyState === WebSocket.OPEN`

**Example - Cancel Order**:
```typescript
ws.send(JSON.stringify({
  mt: 22,
  rq: Date.now(),
  mkt: 16,
  acc: accountId,
  oid: orderIdToCancel,
  t: 5,  // Cancel
  s: 0,
  fl: 0,
  lv: 0,
  lb: currentBlock + 100
}));
```

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
  b: string;        // Balance
  lb: string;       // Locked balance
  h?: AccountEvent[];  // Recent events
}
```

### Keep-Alive

Send periodic pings to keep connection alive:

```typescript
setInterval(() => {
  ws.send(JSON.stringify({
    mt: 1,  // Ping
    t: Date.now()
  }));
}, 30000);
```

### Error Handling

**Close Code 3401**: Authentication failure
- Re-authenticate via REST API
- Reconnect with new nonce

```typescript
ws.onclose = (event) => {
  if (event.code === 3401) {
    // Re-authenticate
    await refreshAuth();
    reconnect();
  } else {
    // Normal reconnection with backoff
    setTimeout(reconnect, getBackoffDelay());
  }
};
```

### Reconnection Strategy

```typescript
const RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 32000, 60000];
let retryCount = 0;

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
```

---

## Sequence Numbers

- `heartbeat` and `gas-stats` streams have continuous sequence numbers
- Other streams may have gaps (e.g., when no activity)
- Track `sn` to detect missed messages
- On gap detection, resubscribe to get fresh snapshot
