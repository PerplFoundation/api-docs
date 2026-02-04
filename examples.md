# Code Examples

Complete examples for common API operations.

## Setup

```typescript
// Load from environment (or use defaults for testnet)
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';
const WS_URL = process.env.PERPL_WS_URL || 'wss://testnet.perpl.xyz';  // Note: WebSocket doesn't use /api prefix
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 10143;

// Market IDs (consistent across networks)
const MARKETS = {
  BTC: 16,
  ETH: 32,
  SOL: 48,
  MON: 64,
  ZEC: 256
} as const;
```

---

## Authentication

### Full Auth Flow

```typescript
import { signMessage } from 'viem/accounts';

async function authenticate(privateKey: `0x${string}`, address: string) {
  // Step 1: Get payload
  const payloadRes = await fetch(`${API_URL}/v1/auth/payload`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ chain_id: CHAIN_ID, address })
  });
  const payload = await payloadRes.json();

  // Step 2: Sign the SIWE message
  const signature = await signMessage({
    message: payload.message,
    privateKey
  });

  // Step 3: Connect (chain_id and address must be included!)
  const connectRes = await fetch(`${API_URL}/v1/auth/connect`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include',
    body: JSON.stringify({
      chain_id: CHAIN_ID,
      address,
      ...payload,
      signature
    })
  });

  if (!connectRes.ok) {
    if (connectRes.status === 418) throw new Error('Access code required');
    if (connectRes.status === 423) throw new Error('Invalid access code');
    if (connectRes.status === 403) throw new Error('Access denied');
    throw new Error(`Auth failed: ${connectRes.status}`);
  }

  const auth = await connectRes.json();
  return auth.nonce;
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
class TradingClient {
  private ws: WebSocket;
  private requestId = Date.now();
  private accountId: number;
  private currentBlock: number = 0;
  private pingInterval?: ReturnType<typeof setInterval>;

  constructor(
    private authNonce: string,
    private onUpdate: (type: string, data: any) => void
  ) {}

  connect() {
    this.ws = new WebSocket(`${WS_URL}/ws/v1/trading`);

    this.ws.onopen = () => {
      // Authenticate
      this.ws.send(JSON.stringify({
        mt: 4,
        chain_id: CHAIN_ID,
        nonce: this.authNonce,
        ses: crypto.randomUUID()
      }));
    };

    this.ws.onmessage = (event) => {
      const msg = JSON.parse(event.data);

      switch (msg.mt) {
        case 19: // WalletSnapshot
          this.accountId = msg.as?.[0]?.id;
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

  async openLong(
    marketId: number,
    size: number,
    price: number | null,
    leverage: number
  ) {
    const order = {
      mt: 22,
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: 1, // OpenLong
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4, // GTC for limit, IOC for market
      lv: leverage * 100,
      lb: this.currentBlock + 100
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
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: 2, // OpenShort
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4,
      lv: leverage * 100,
      lb: this.currentBlock + 100
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
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      t: isLong ? 3 : 4, // CloseLong or CloseShort
      p: price ?? 0,
      s: size,
      fl: price ? 0 : 4,
      lp: positionId,
      lv: 0,
      lb: this.currentBlock + 100
    };

    this.ws.send(JSON.stringify(order));
    return order.rq;
  }

  async cancelOrder(marketId: number, orderId: number) {
    const order = {
      mt: 22,
      rq: this.nextRequestId(),
      mkt: marketId,
      acc: this.accountId,
      oid: orderId,
      t: 5, // Cancel
      s: 0,
      fl: 0,
      lv: 0,
      lb: this.currentBlock + 100
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
const nonce = await authenticate(privateKey, address);
const client = new TradingClient(nonce, (type, data) => {
  console.log(type, data);
});
client.connect();

// Wait for connection and snapshots...
setTimeout(async () => {
  // Open 0.1 BTC long at market price with 10x leverage
  // Note: size needs to be scaled (5 decimals for BTC)
  const requestId = await client.openLong(MARKETS.BTC, 10000, null, 10);
  console.log('Order submitted:', requestId);
}, 2000);
```

---

## Fetching History

### Get All Fills

```typescript
async function getAllFills(authNonce: string): Promise<any[]> {
  const fills: any[] = [];
  let cursor: string | undefined;

  do {
    const url = new URL(`${API_URL}/v1/trading/fills`);
    if (cursor) url.searchParams.set('page', cursor);
    url.searchParams.set('count', '100');

    const res = await fetch(url.toString(), {
      headers: { 'X-Auth-Nonce': authNonce },
      credentials: 'include'
    });

    const data = await res.json();
    fills.push(...data.d);
    cursor = data.np;
  } while (cursor);

  return fills;
}
```

### Get Position PnL History

```typescript
async function getPositionHistory(authNonce: string): Promise<any[]> {
  const positions: any[] = [];
  let cursor: string | undefined;

  do {
    const url = new URL(`${API_URL}/v1/trading/position-history`);
    if (cursor) url.searchParams.set('page', cursor);
    url.searchParams.set('count', '50');

    const res = await fetch(url.toString(), {
      headers: { 'X-Auth-Nonce': authNonce },
      credentials: 'include'
    });

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
      // Auth expired - need to re-authenticate
      console.error('Auth expired, please re-authenticate');
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
