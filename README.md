# Perpl API Documentation

API documentation for the Perpl perpetual futures exchange on Monad.

## Overview

The Perpl API provides two communication channels:

| Channel | Protocol | Purpose | Auth Required |
|---------|----------|---------|---------------|
| REST API | HTTPS | History queries, authentication, profile | Varies |
| WebSocket | WSS | Real-time data, trading | Varies |

**Base URL**: Configured via environment (see [Configuration](#configuration))

## Quick Start

### 1. Get Market Data (No Auth)

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';

// Fetch context (markets, tokens, chain config)
const context = await fetch(`${API_URL}/v1/pub/context`)
  .then(r => r.json());

console.log(context.markets); // Available markets
console.log(context.chain);   // Chain configuration
```

### 2. Connect to Market Data WebSocket

```typescript
const WS_URL = process.env.PERPL_WS_URL || 'wss://testnet.perpl.xyz';

const ws = new WebSocket(`${WS_URL}/ws/v1/market-data`);

ws.onopen = () => {
  // Subscribe to BTC order book (market_id=16)
  ws.send(JSON.stringify({
    mt: 5, // MsgTypeSubscriptionRequest
    subs: [{ stream: 'order-book@16', subscribe: true }]
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Message type:', msg.mt);
};
```

### 3. Authenticate (Required for Trading)

> **Note**: Authentication requires a **whitelisted wallet**. Non-whitelisted wallets will receive HTTP 418 (Access code required). See [Wallet Requirements](#wallet-requirements) below.

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 10143;
const address = '0xYourWalletAddress';

// Step 1: Get signing payload
const payload = await fetch(`${API_URL}/v1/auth/payload`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address
  })
}).then(r => r.json());

// Step 2: Sign the SIWE message with your wallet
const signature = await wallet.signMessage({ message: payload.message });

// Step 3: Connect with signature (chain_id and address required!)
const auth = await fetch(`${API_URL}/v1/auth/connect`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address,
    ...payload,
    signature
  })
}).then(r => r.json());

// auth.nonce is used for authenticated requests
```

## API Reference

| Document | Description |
|----------|-------------|
| [Authentication](./authentication.md) | Wallet signature auth flow |
| [REST Endpoints](./rest-endpoints.md) | All HTTP endpoints |
| [WebSocket](./websocket.md) | Real-time streams and trading |
| [Types](./types.md) | Data type reference |
| [Examples](./examples.md) | Code examples |

## Configuration

All URLs and chain settings are configurable via environment variables. Copy `.env.example` to `.env`:

```bash
cp .env.example .env
```

| Variable | Default | Description |
|----------|---------|-------------|
| `PERPL_API_URL` | `https://testnet.perpl.xyz/api` | REST API base URL |
| `PERPL_WS_URL` | `wss://testnet.perpl.xyz` | WebSocket base URL |
| `PERPL_CHAIN_ID` | `10143` | Chain ID |
| `PERPL_RPC_URL` | `https://testnet-rpc.monad.xyz` | RPC URL for on-chain ops |
| `PERPL_EXCHANGE_ADDRESS` | `0x9c216d...` | Exchange contract |
| `PERPL_COLLATERAL_TOKEN` | `0xdf5b71...` | USD collateral token |

## Chain Configuration (Testnet)

| Property | Value |
|----------|-------|
| Chain ID | 10143 |
| Network | Monad Testnet |
| Exchange Contract | `0x9c216d1ab3e0407b3d6f1d5e9effe6d01c326ab7` |
| Collateral Token | `0xdf5b718d8fcc173335185a2a1513ee8151e3c027` (USD) |
| RPC URL | `https://testnet-rpc.monad.xyz` |

## Markets

| Market ID | Symbol | Perp ID |
|-----------|--------|---------|
| 16 | BTC | 16 |
| 32 | ETH | 32 |
| 48 | SOL | 48 |
| 64 | MON | 64 |
| 256 | ZEC | 256 |

## Rate Limits

| Endpoint Type | Estimated Limit | Notes |
|---------------|-----------------|-------|
| REST Public | ~100 req/min | `/api/v1/pub/*`, market data |
| REST Authenticated | ~60 req/min | Profile, trading history |
| WebSocket Messages | ~50 msg/sec | Per connection |
| WebSocket Connections | ~5 per IP | Market data + trading |

**Rate Limit Response**: HTTP 429 Too Many Requests

```typescript
// Handle rate limiting with exponential backoff
async function fetchWithRetry(url: string, options: RequestInit, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    const res = await fetch(url, options);
    if (res.status === 429) {
      const delay = Math.pow(2, i) * 1000; // 1s, 2s, 4s
      await new Promise(r => setTimeout(r, delay));
      continue;
    }
    return res;
  }
  throw new Error('Rate limit exceeded after retries');
}
```

*Note: Actual limits may vary. Monitor for 429 responses and adjust request frequency accordingly.*

## Error Handling

### HTTP Status Codes

| Code | Meaning |
|------|---------|
| 200 | Success |
| 400 | Bad Request |
| 403 | Forbidden - Access denied |
| 404 | Not Found |
| 418 | Access code required |
| 423 | Access code invalid/exhausted |
| 429 | Too Many Requests |
| 500 | Internal Server Error |

### WebSocket Close Codes

| Code | Meaning |
|------|---------|
| 3401 | Unauthorized - Authentication failure |

## Wallet Requirements

### Whitelisted Wallet Required

The Perpl testnet requires a **whitelisted wallet** to access authenticated endpoints. When authenticating with a non-whitelisted wallet, you'll receive:

- **HTTP 418**: Access code required - wallet is not on the whitelist
- **HTTP 423**: Access code invalid/exhausted - referral code was invalid

### Public Endpoints (No Wallet Required)

These endpoints work without authentication:

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/pub/context` | Chain and market configuration |
| `GET /api/v1/market-data/.../candles/...` | OHLCV candlestick data |
| `GET /api/v1/profile/announcements` | Public announcements |
| `wss://.../ws/v1/market-data` | Real-time market data streams |

### Authenticated Endpoints (Whitelisted Wallet Required)

These endpoints require authentication with a whitelisted wallet:

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/trading/account-history` | Account events (deposits, settlements, etc.) |
| `GET /api/v1/trading/fills` | Order fill history |
| `GET /api/v1/trading/order-history` | Order history |
| `GET /api/v1/trading/position-history` | Position history |
| `GET /api/v1/profile/ref-code` | Your referral code |
| `GET /api/v1/profile/contact-info` | Your contact info |
| `wss://.../ws/v1/trading` | Real-time trading data |

### Testing with a Wallet

To test authenticated endpoints, provide your wallet private key via environment variable:

```bash
# In your .env file
OWNER_PRIVATE_KEY=0x...your_private_key_here...
```

Then run the API test script:

```bash
npm run test:api
```

Or test all authenticated endpoints:

```bash
npx tsx scripts/test-auth-endpoints.ts
```

### Getting Whitelisted

To get your wallet whitelisted on Perpl testnet:

1. Visit [testnet.perpl.xyz](https://testnet.perpl.xyz)
2. Connect your wallet
3. Request access or use a referral code if available
4. Once approved, your wallet can authenticate via the API
