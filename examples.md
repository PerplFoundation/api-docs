# Code Examples

Complete examples for common API operations.

## Setup

```typescript
// Load from environment (or use defaults for mainnet)
const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';  // Note: WebSocket doesn't use /api prefix
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

// Market IDs for mainnet
// Note: Testnet uses different IDs (BTC=16, ETH=32, SOL=48, MON=64, ZEC=256)
const MARKETS = {
  BTC: 1,
  MON: 10,
  ETH: 20,
  SOL: 31,
  HYPE: 40,
  ZEC: 50,
} as const;
```

---

## Authentication

Perpl authenticates programmatic clients with **API keys** (an Ed25519 key
pair). Every request is signed with the key's private key — there is no bearer
token or session cookie.

### Getting a key

Create a key with the web UI — connect your wallet at:

- **Mainnet**: https://app.perpl.xyz/apikeys
- **Testnet**: https://testnet.perpl.xyz/apikeys

The UI walks you through the wallet-signed enrollment and hands you the private
key (`privateKey`, Ed25519) plus the `API_KEY` token (`X-API-Key`). Third-party
integrations can also enroll keys programmatically — see
[Integrations](./integrations.md); a runnable JS enrollment example lives at
`examples/js/enroll_api_key.js`. That example enrolls a **builder-bound** key
when `PERPL_BUILDER_ID` (and optionally `PERPL_MAX_BUILDER_FEE_PER_100K`) is set
— see [Builder codes](./integrations.md#builder-codes) for what those mean and
how to charge the fee with `bf` on an order.

The runnable example programs read an already-enrolled key from the environment:

```typescript
import * as ed from '@noble/ed25519';

const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

// Provide an enrolled key via the environment:
//   PERPL_API_KEY        — the X-API-Key token
//   PERPL_API_KEY_SECRET — hex of the 32-byte Ed25519 private key
const API_KEY = process.env.PERPL_API_KEY!;
const privateKey = Buffer.from(process.env.PERPL_API_KEY_SECRET!.replace(/^0x/, ''), 'hex');
```

### Signed REST requests

Every REST call is signed with a `signedFetch` helper. It builds the canonical
string, signs it with the key, and sends the four `X-API-*` headers (see
[authentication.md](./authentication.md#authenticating-rest-requests)).

```typescript
import { createHash, randomBytes } from 'crypto';
import * as ed from '@noble/ed25519';

// `target` is the path + query string exactly as sent, e.g. /v1/trading/fills?count=100
async function signedFetch(method: string, target: string, body = '') {
  const timestamp = Date.now().toString();
  const nonce = randomBytes(16).toString('base64url');
  const bodyHash = createHash('sha256').update(body).digest('hex');

  const canonical = [CHAIN_ID, method, target, timestamp, nonce, bodyHash].join('\n');
  const sig = await ed.signAsync(Buffer.from(canonical), privateKey);

  return fetch(`${API_URL}${target}`, {
    method,
    headers: {
      'X-API-Key': API_KEY,
      'X-API-Timestamp': timestamp,
      'X-API-Nonce': nonce,
      'X-API-Signature': Buffer.from(sig).toString('base64url'),
      ...(body ? { 'Content-Type': 'application/json' } : {}),
    },
    ...(body ? { body } : {}),
  });
}
```

---

## Fetching Market Data

### Get Context

```typescript
async function getContext() {
  const res = await fetch(`${API_URL}/v1/pub/context`);
  const context = await res.json();

  return {
    markets: new Map(context.markets.map(m => [m.id, m])),
    tokens: new Map(context.tokens.map(t => [t.id, t])),
    instances: new Map(context.instances.map(i => [i.id, i]))
  };
}
```

### Get Candles

```typescript
async function getCandles(
  marketId: number,
  resolution: number,
  hours: number = 24
) {
  const to = Date.now();
  const from = to - (hours * 60 * 60 * 1000);

  const res = await fetch(
    `${API_URL}/v1/market-data/${marketId}/candles/${resolution}/${from}-${to}`
  );
  const data = await res.json();

  // Get market config for scaling
  const context = await getContext();
  const market = context.markets.get(marketId);
  const priceScale = Math.pow(10, market.config.price_decimals);

  return data.d.map(c => ({
    time: c.t,
    open: c.o / priceScale,
    high: c.h / priceScale,
    low: c.l / priceScale,
    close: c.c / priceScale,
    volume: parseFloat(c.v),
    trades: c.n
  }));
}

// Usage
const btcCandles = await getCandles(MARKETS.BTC, 3600, 24); // 1h candles, 24 hours
```

### Get Funding History

```typescript
async function getFundingHistory(marketId: number, hours: number = 24) {
  const to = Date.now();
  const from = to - (hours * 60 * 60 * 1000);

  const res = await fetch(
    `${API_URL}/v1/market-data/${marketId}/funding/${from}-${to}`
  );
  const data = await res.json();

  // Get market config for scaling
  const context = await getContext();
  const market = context.markets.get(marketId);
  const priceScale = Math.pow(10, market.config.price_decimals);

  // `at.t` is when the rate applies, not when it was published, so it is the
  // series' time axis. Events are keyed by `feb` (the funding interval): a repeat
  // of one already seen updates that entry rather than adding a point.
  return data.d.map(f => ({
    time: f.at.t,
    interval: f.feb,
    ratePct: f.rate / 10_000,          // micros -> percent
    indexPrice: f.idx / priceScale,
    paymentPerLot: f.ppl / priceScale
  }));
}

// Usage
const btcFunding = await getFundingHistory(MARKETS.BTC, 24 * 7); // last week
```

### Get Funding History of All Markets

```typescript
async function getAllFundingHistory(hours: number = 24) {
  const to = Date.now();
  const from = to - (hours * 60 * 60 * 1000);

  const res = await fetch(`${API_URL}/v1/market-data/funding/${from}-${to}`);
  const data = await res.json();

  // Get market configs for scaling
  const context = await getContext();

  // `d` is keyed by market ID; a market with no funding history is absent from
  // it, and the number of events per market follows each market's own funding
  // interval
  return new Map(Object.entries(data.d).map(([marketId, events]) => {
    const market = context.markets.get(Number(marketId));
    const priceScale = Math.pow(10, market.config.price_decimals);

    return [Number(marketId), events.map(f => ({
      time: f.at.t,
      interval: f.feb,
      ratePct: f.rate / 10_000,          // micros -> percent
      indexPrice: f.idx / priceScale,
      paymentPerLot: f.ppl / priceScale
    }))];
  }));
}

// Usage: the period is capped at 128 intervals of the market funded most often
const funding = await getAllFundingHistory(24);
const btcFundingToday = funding.get(MARKETS.BTC);
```

---

## Market Data WebSocket

### Order Book Subscription

```typescript
class OrderBookClient {
  private ws: WebSocket;
  private bids: Map<number, { size: number; orders: number }> = new Map();
  private asks: Map<number, { size: number; orders: number }> = new Map();

  constructor(private marketId: number) {}

  connect() {
    this.ws = new WebSocket(`${WS_URL}/ws/v1/market-data`);

    this.ws.onopen = () => {
      this.ws.send(JSON.stringify({
        mt: 5,
        subs: [{ stream: `order-book@${this.marketId}`, subscribe: true }]
      }));
    };

    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);

      if (msg.mt === 15) {
        // Snapshot
        this.bids.clear();
        this.asks.clear();
        this.applyLevels(msg.bid, this.bids);
        this.applyLevels(msg.ask, this.asks);
      } else if (msg.mt === 16) {
        // Update
        this.applyLevels(msg.bid, this.bids);
        this.applyLevels(msg.ask, this.asks);
      }
    };
  }

  private applyLevels(
    levels: Array<{ p: number; s: number; o: number }>,
    book: Map<number, { size: number; orders: number }>
  ) {
    for (const level of levels) {
      if (level.o === 0) {
        book.delete(level.p);
      } else {
        book.set(level.p, { size: level.s, orders: level.o });
      }
    }
  }

  getBestBid(): number | undefined {
    const prices = [...this.bids.keys()].sort((a, b) => b - a);
    return prices[0];
  }

  getBestAsk(): number | undefined {
    const prices = [...this.asks.keys()].sort((a, b) => a - b);
    return prices[0];
  }

  disconnect() {
    this.ws?.close();
  }
}

// Usage
const book = new OrderBookClient(MARKETS.BTC);
book.connect();
```

### Trade Stream

```typescript
function subscribeToTrades(marketId: number, onTrade: (trade: any) => void) {
  const ws = new WebSocket(`${WS_URL}/ws/v1/market-data`);

  ws.onopen = () => {
    ws.send(JSON.stringify({
      mt: 5,
      subs: [{ stream: `trades@${marketId}`, subscribe: true }]
    }));
  };

  ws.onmessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.mt === 17 || msg.mt === 18) {
      for (const trade of msg.d) {
        onTrade({
          price: trade.p,
          size: trade.s,
          side: trade.sd === 1 ? 'buy' : 'sell',
          timestamp: trade.at.t,
          block: trade.at.b
        });
      }
    }
  };

  return () => ws.close();
}
```

---

## Trading WebSocket

### Trading Client

```typescript
import { randomBytes } from 'crypto';
import * as ed from '@noble/ed25519';

class TradingClient {
  private ws: WebSocket;
  private requestId = 0;  // seeded from account.lfr on WalletSnapshot
  private sn = 0;         // unique, non-zero per outbound frame; echoed as `cid` on mt: 3
  private accountId: number;
  private currentBlock: number = 0;
  private lastSn?: number;
  private pingInterval?: ReturnType<typeof setInterval>;
  private marketTtl: Map<number, number> = new Map(); // marketId -> order_ttl_blocks

  constructor(
    private privateKey: Uint8Array,
    private apiKey: string,
    private onUpdate: (type: string, data: any) => void
  ) {}

  // Call once before connect(). order_ttl_blocks is per-market and can change,
  // so read it at runtime rather than hardcoding a value.
  async loadMarkets() {
    const context = await getContext();
    for (const market of context.markets.values()) {
      this.marketTtl.set(market.id, market.order_ttl_blocks);
    }
  }

  connect() {
    this.ws = new WebSocket(`${WS_URL}/ws/v1/trading`);

    this.ws.onopen = async () => {
      // Authenticate: signed ApiKeySignIn frame as the first message.
      const timestamp = Date.now().toString();
      const nonce = randomBytes(16).toString('base64url');
      const canonical = [CHAIN_ID, 'trading-ws-signin', timestamp, nonce].join('\n');
      const sig = await ed.signAsync(Buffer.from(canonical), this.privateKey);

      this.ws.send(JSON.stringify({
        mt: 29, // ApiKeySignIn
        chain_id: CHAIN_ID,
        api_key: this.apiKey,
        timestamp,
        nonce,
        signature: Buffer.from(sig).toString('base64url')
      }));
    };

    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);

      switch (msg.mt) {
        case 3: // StatusResponse — one per mt: 22 frame
          // msg.cid is the `sn` we sent (omitted if that sn was 0).
          // code 0 = accepted for forwarding, not filled; outcome comes via mt: 24.
          if (msg.status?.code === 0) {
            this.onUpdate('orderAccepted', msg);
          } else {
            // Rejected at the gateway — no mt: 24 will follow for this frame.
            console.warn('Order rejected:', { cid: msg.cid, status: msg.status });
            this.onUpdate('orderRejected', msg);
          }
          break;
        case 19: // WalletSnapshot
          this.accountId = msg.as?.[0]?.id;
          // rq = max(localCounter, account.lfr) + 1
          this.requestId = Math.max(this.requestId, msg.as?.[0]?.lfr ?? 0);
          this.lastSn = msg.sn; // Initialize sequence tracking
          this.onUpdate('wallet', msg);
          break;
        case 23: // OrdersSnapshot
          this.onUpdate('orders', msg.d);
          break;
        case 24: // OrdersUpdate
          this.onUpdate('orderUpdate', msg.d);
          break;
        case 25: // FillsUpdate
          this.onUpdate('fills', msg.d);
          break;
        case 26: // PositionsSnapshot
          this.onUpdate('positions', msg.d);
          break;
        case 27: // PositionsUpdate
          this.onUpdate('positionUpdate', msg.d);
          break;
        case 100: // Heartbeat
          if (this.lastSn != null && msg.sn !== this.lastSn + 1) {
            console.warn('Sequence gap, reconnecting...');
            this.disconnect();
            this.connect();
            return;
          }
          this.lastSn = msg.sn;
          this.currentBlock = msg.h;
          break;
      }
    };

    // Keep alive - store reference for cleanup
    this.pingInterval = setInterval(() => {
      if (this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ mt: 1, t: Date.now() }));
      }
    }, 30000);
  }

  private nextRequestId(): number {
    return ++this.requestId;
  }

  private nextSn(): number {
    return ++this.sn;
  }

  private lastExecBlock(marketId: number): number {
    const ttl = this.marketTtl.get(marketId);
    if (ttl == null) {
      throw new Error(`Unknown market ${marketId} — order_ttl_blocks not loaded`);
    }
    return this.currentBlock + ttl;
  }

  async openLong(
    marketId: number,
    size: number,
    price: number | null,
    leverage: number
  ) {
    const order = {
      mt: 22,
      sn: this.nextSn(),      // unique, non-zero; correlates the mt: 3 status
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: 1, // OpenLong
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4, // GTC for limit, IOC for market
      lv: leverage * 100,
      lb: this.lastExecBlock(marketId)
    };

    this.ws.send(JSON.stringify(order));
    return order.rq;
  }

  async openShort(
    marketId: number,
    size: number,
    price: number | null,
    leverage: number
  ) {
    const order = {
      mt: 22,
      sn: this.nextSn(),
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: 2, // OpenShort
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4,
      lv: leverage * 100,
      lb: this.lastExecBlock(marketId)
    };

    this.ws.send(JSON.stringify(order));
    return order.rq;
  }

  async closePosition(
    marketId: number,
    positionId: number,
    size: number,
    isLong: boolean,
    price: number | null
  ) {
    const order = {
      mt: 22,
      sn: this.nextSn(),
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: isLong ? 3 : 4, // CloseLong or CloseShort
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4,
      lp: positionId,
      lv: 0,
      lb: this.lastExecBlock(marketId)
    };

    this.ws.send(JSON.stringify(order));
    return order.rq;
  }

  async cancelOrder(marketId: number, orderId: number) {
    const order = {
      mt: 22,
      sn: this.nextSn(),
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      oid: orderId,
      t: 5, // Cancel
      s: 0,
      fl: 0,
      lv: 0,
      lb: this.lastExecBlock(marketId)
    };

    this.ws.send(JSON.stringify(order));
    return order.rq;
  }

  disconnect() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = undefined;
    }
    this.ws?.close();
  }
}

// Usage
// privateKey + API_KEY come from the enrolled key (see Authentication above).
const client = new TradingClient(privateKey, API_KEY, (type, data) => {
  console.log(type, data);
});
await client.loadMarkets();  // per-market order_ttl_blocks, for computing `lb`
client.connect();

// Wait for connection and snapshots...
setTimeout(async () => {
  // Open 0.1 BTC long at market price with 10x leverage
  // Note: size needs to be scaled (5 decimals for BTC on mainnet)
  const requestId = await client.openLong(MARKETS.BTC, 10000, null, 10);
  console.log('Order submitted:', requestId);
}, 2000);
```

---

## Fetching History

### Get All Fills

```typescript
async function getAllFills(): Promise<any[]> {
  const fills: any[] = [];
  let cursor: string | undefined;

  do {
    // Build the query string; the signature binds the full request target.
    const params = new URLSearchParams();
    if (cursor) params.set('page', cursor);
    params.set('count', '100');
    const target = `/v1/trading/fills?${params.toString()}`;

    const res = await signedFetch('GET', target);

    const data = await res.json();
    fills.push(...data.d);
    cursor = data.np;
  } while (cursor);

  return fills;
}
```

### Get Position PnL History

```typescript
async function getPositionHistory(): Promise<any[]> {
  const positions: any[] = [];
  let cursor: string | undefined;

  do {
    // Build the query string; the signature binds the full request target.
    const params = new URLSearchParams();
    if (cursor) params.set('page', cursor);
    params.set('count', '50');
    const target = `/v1/trading/position-history?${params.toString()}`;

    const res = await signedFetch('GET', target);

    const data = await res.json();
    positions.push(...data.d);
    cursor = data.np;
  } while (cursor);

  return positions;
}
```

---

## Utility Functions

### Price Conversion

```typescript
function createPriceConverter(priceDecimals: number) {
  const scale = Math.pow(10, priceDecimals);

  return {
    toScaled: (price: number) => Math.round(price * scale),
    fromScaled: (scaled: number) => scaled / scale
  };
}

// BTC has 1 price decimal
const btcPrice = createPriceConverter(1);
console.log(btcPrice.toScaled(95000));    // 950000
console.log(btcPrice.fromScaled(950000)); // 95000
```

### Size Conversion

```typescript
function createSizeConverter(sizeDecimals: number) {
  const scale = Math.pow(10, sizeDecimals);

  return {
    toScaled: (size: number) => Math.round(size * scale),
    fromScaled: (scaled: number) => scaled / scale
  };
}

// BTC has 5 size decimals
const btcSize = createSizeConverter(5);
console.log(btcSize.toScaled(0.1));    // 10000
console.log(btcSize.fromScaled(10000)); // 0.1
```

### Leverage Conversion

```typescript
// Leverage is stored in hundredths
const leverageToHundredths = (lev: number) => lev * 100;
const hundredthsToLeverage = (h: number) => h / 100;

console.log(leverageToHundredths(10));  // 1000
console.log(hundredthsToLeverage(1000)); // 10
```

---

## Error Handling

```typescript
async function safeApiCall<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn();
  } catch (error) {
    if (error.response?.status === 429) {
      // Rate limited - wait and retry
      await new Promise(r => setTimeout(r, 1000));
      return safeApiCall(fn);
    }
    throw error;
  }
}

function handleWebSocketError(ws: WebSocket, onReconnect: () => void) {
  const RETRY_DELAYS = [1000, 2000, 4000, 8000, 16000, 32000, 60000];
  let retries = 0;

  ws.onclose = (event) => {
    if (event.code === 3401) {
      // WebSocket auth failed - reconnect and re-send a fresh signed
      // mt:29 ApiKeySignIn frame (new timestamp + nonce).
      console.error('WebSocket auth failed, reconnecting to re-sign in');
      onReconnect();
      return;
    }

    const delay = RETRY_DELAYS[Math.min(retries++, RETRY_DELAYS.length - 1)];
    console.log(`Reconnecting in ${delay}ms...`);
    setTimeout(onReconnect, delay);
  };

  ws.onerror = (error) => {
    console.error('WebSocket error:', error);
  };
}
```
