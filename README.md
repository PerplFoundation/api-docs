# Perpl API Documentation

API documentation for the Perpl perpetual futures exchange on Monad.

## Overview

The Perpl API provides two communication channels:

| Channel | Protocol | Purpose | Auth Required |
|---------|----------|---------|---------------|
| REST API | HTTPS | History queries, authentication, profile | Varies |
| WebSocket | WSS | Real-time data, trading | Varies |

**Base URL**: Configured via environment (see [Configuration](#configuration))

Code examples for JavaScript, Rust, Python and TypeScript can be found in examples/

## Quick Start

### 1. Get Market Data (No Auth)

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';

// Fetch context (markets, tokens, chain config)
const context = await fetch(`${API_URL}/v1/pub/context`)
  .then(r => r.json());

console.log(context.markets); // Available markets
console.log(context.chain);   // Chain configuration
```

### 2. Connect to Market Data WebSocket

```typescript
const WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';

const ws = new WebSocket(`${WS_URL}/ws/v1/market-data`);

ws.onopen = () => {
  // Subscribe to BTC order book (market_id=1 on mainnet)
  ws.send(JSON.stringify({
    mt: 5, // MsgTypeSubscriptionRequest
    subs: [{ stream: 'order-book@1', subscribe: true }]
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  console.log('Message type:', msg.mt);
};
```

### 3. Authenticate (Required for Trading)

Programmatic clients authenticate with **API keys** (an Ed25519 key pair). You
enroll the public key once — authorized by a one-time wallet signature — and
then sign every request with the private key. There is no session cookie or
bearer token.

Create a key from the web UI (**mainnet** [app.perpl.xyz/apikeys](https://app.perpl.xyz/apikeys),
**testnet** [testnet.perpl.xyz/apikeys](https://testnet.perpl.xyz/apikeys)).
Third-party integrations can enroll keys programmatically — see
**[Integrations](./integrations.md)**. For how to sign each request with a key,
see **[Authentication](./authentication.md)**.

```typescript
import * as ed from '@noble/ed25519';
import { createHash, randomBytes } from 'crypto';

const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

// An enrolled key (from the web UI or Integrations enrollment), read from the environment:
const API_KEY = process.env.PERPL_API_KEY;                    // the opaque X-API-Key token
const privateKey = Buffer.from((process.env.PERPL_API_KEY_SECRET ?? '').replace(/^0x/, ''), 'hex'); // Ed25519 private key

// Sign the canonical string and send the four X-API-* headers.
const target = '/v1/trading/fills?count=1';
const timestamp = Date.now().toString();
const nonce = randomBytes(16).toString('base64url');
const bodyHash = createHash('sha256').update('').digest('hex');
const canonical = [CHAIN_ID, 'GET', target, timestamp, nonce, bodyHash].join('\n');
const sig = await ed.signAsync(Buffer.from(canonical), privateKey);

const response = await fetch(`${API_URL}${target}`, {
   headers: {
      'X-API-Key': API_KEY,
      'X-API-Timestamp': timestamp,
      'X-API-Nonce': nonce,
      'X-API-Signature': Buffer.from(sig).toString('base64url'),
   },
});

console.log(await response.json());
```

> **Important**: Successful API authentication does NOT mean you have an exchange account.
> See [API Auth vs Smart Contract Account](#api-auth-vs-smart-contract-account) below.
> Some calls will return 404 if a Smart Contract Account has not been created.

## API Reference

| Document | Description |
|----------|-------------|
| [Authentication](./authentication.md) | Signing requests with an API key |
| [Integrations](./integrations.md) | API key enrollment for third-party services |
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
| `PERPL_API_URL` | `https://app.perpl.xyz/api` | REST API base URL |
| `PERPL_WS_URL` | `wss://app.perpl.xyz` | WebSocket base URL |
| `PERPL_CHAIN_ID` | `143` | Chain ID |
| `PERPL_RPC_URL` | `https://rpc.monad.xyz` | RPC URL for on-chain ops |
| `PERPL_EXCHANGE_ADDRESS` | `0x34B6552d...` | Exchange contract |
| `PERPL_COLLATERAL_TOKEN` | `0x00000000eF...` | AUSD collateral token |

## Network Configuration

Perpl runs on both **Mainnet** (default) and **Testnet**.

| | Mainnet (default) | Testnet |
|---|---|---|
| REST API | `https://app.perpl.xyz/api` | `https://testnet.perpl.xyz/api` |
| WebSocket | `wss://app.perpl.xyz` | `wss://testnet.perpl.xyz` |
| Chain ID | `143` | `10143` |
| RPC | `https://rpc.monad.xyz` | `https://testnet-rpc.monad.xyz` |
| Exchange | `0x34B6552d57a35a1D042CcAe1951BD1C370112a6F` | `0x1964c32f0be608e7d29302aff5e61268e72080cc` |
| Collateral | `0x00000000eFE302BEAA2b3e6e1b18d08D69a9012a` (AUSD) | `0xdf5b718d8fcc173335185a2a1513ee8151e3c027` (USD) |

To use testnet, set the environment variables to testnet values.

## Markets

Market IDs differ between networks:

**Mainnet**:
| Market ID | Symbol |
|-----------|--------|
| 1 | BTC |
| 10 | MON |
| 20 | ETH |
| 31 | SOL |
| 40 | HYPE |
| 50 | ZEC |

**Testnet**:
| Market ID | Symbol |
|-----------|--------|
| 16 | BTC |
| 32 | ETH |
| 48 | SOL |
| 64 | MON |
| 256 | ZEC |

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
| 401 | Unauthorized - bad/stale signature, replayed nonce, or revoked/expired key |
| 403 | Forbidden - scope insufficient |
| 404 | Not Found |
| 429 | Too Many Requests |
| 500 | Internal Server Error |

### WebSocket Close Codes

| Code | Meaning |
|------|---------|
| 3401 | Unauthorized - Authentication failure |

## Endpoint Authentication

### Public Endpoints (No Auth)

These endpoints work without authentication:

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/pub/context` | Chain and market configuration |
| `GET /api/v1/market-data/.../candles/...` | OHLCV candlestick data |
| `GET /api/v1/profile/announcements` | Public announcements |
| `wss://.../ws/v1/market-data` | Real-time market data streams |

### Authenticated Endpoints (API Key)

These endpoints require a signed request from an enrolled API key (see [Authentication](./authentication.md)):

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/trading/account-history` | Account events (deposits, settlements, etc.) |
| `GET /api/v1/trading/fills` | Order fill history |
| `GET /api/v1/trading/order-history` | Order history |
| `GET /api/v1/trading/position-history` | Position history |
| `GET /api/v1/profile/ref-code` | Your referral code |
| `wss://.../ws/v1/trading` | Real-time trading data & order placement (`trade` scope) |

### Testing

Create a key at the web UI ([app.perpl.xyz/apikeys](https://app.perpl.xyz/apikeys),
testnet [testnet.perpl.xyz/apikeys](https://testnet.perpl.xyz/apikeys)) or enroll
one programmatically, then run the examples in `examples/` (see
[Examples](./examples.md)). The example programs read your enrolled key from
`PERPL_API_KEY` / `PERPL_API_KEY_SECRET`, and the wallet used for enrollment
from a private key you supply.

## API Auth vs Smart Contract Account

**This is a common source of confusion.** API authentication and smart contract account creation are completely separate:

| Concept | What It Means | Required For |
|---------|---------------|--------------|
| **API Authentication** | An enrolled API key can call authenticated API endpoints | Reading order history, position history, trading WebSocket |
| **Exchange Account** | On-chain account exists on Exchange contract with collateral | Placing orders, holding positions, trading |

### Key Points

1. **API auth does NOT create an exchange account**
   - Enrolling an API key only authorizes API access for your wallet
   - A valid signed request means you can use authenticated API endpoints
   - It does NOT mean you can trade

2. **Exchange account must be created on-chain**
   - Call `createAccount(uint256 amountCNS)` on the Exchange contract
   - Requires initial collateral deposit (USDC)
   - This creates your account ID and enables trading

3. **Both are required for full functionality**
   - API auth → Access trading history, real-time data via authenticated endpoints
   - Exchange account → Actually place orders and hold positions

On the front end, the "Deposit to Enable trading" button takes care of this flow.

### Fetch smart contract information

Fetch the smart contract information from the public API over http:

```typescript
    const accountCreationInfo = await getAccountCreationInfo(API_URL);
    console.log(accountCreationInfo);
    // Example output:
    // { 'account_open_min_deposit_display': '10.0 AUSD',
    //   'collateral_token_address': '0x00000000efe302beaa2b3e6e1b18d08d69a9012a',
    //   'collateral_token_symbol': 'AUSD',
    //   'min_account_open_amount': 100000000,
    //   'smart_contract_address': '0xSMART_CONTRACT_ADDRESS'
    // }
```

Example code is given in examples/*/fetch_smart_contract_info for JavaScript, Python, Rust and TypeScript.

### Checking Account Status

To make the API calls to the Smart Contract use Foundry: https://www.getfoundry.sh and fill in the environment variables with the information from the public API.

```bash
export SMART_CONTRACT_ADDRESS=0xSmartContractAddress
export WALLET_ADDRESS=0xYourWalletAddress
cast call --from $WALLET_ADDRESS $SMART_CONTRACT_ADDRESS "getAccountByAddr(address)(uint256)" $WALLET_ADDRESS --rpc-url $RPC_URL
```

This call returns the account id for an address if an account exists otherwise returns an execution reverted error.

### Creating an Exchange Account

Again, fill in the environment variables using the API output above and use cast to create the Smart Contract Account:

```bash
export SMART_CONTRACT_ADDRESS=0xSmartContractAddress
export TOKEN_CONTRACT_ADDRESS=0xCollateralTokenContractAddress
export WALLET_ADDRESS=0xYourWalletAddress
export WALLET_KEY=0xYourWalletPrivateKey;
# Check API for current min account open amount
export MIN_ACCOUNT_OPEN_AMOUNT=100000000

# Approve deposit to DEX contract (ERC-20)
cast send --from $WALLET_ADDRESS $TOKEN_CONTRACT_ADDRESS "approve(address,uint256)(bool)" $SMART_CONTRACT_ADDRESS $MIN_ACCOUNT_OPEN_AMOUNT --private-key $WALLET_KEY --rpc-url $RPC_URL

# Create DEX account with initial deposit
cast send --from $WALLET_ADDRESS $SMART_CONTRACT_ADDRESS "createAccount(uint256)(uint256)" $MIN_ACCOUNT_OPEN_AMOUNT --private-key $WALLET_KEY --rpc-url $RPC_URL
```

### Common Error Scenarios

| Symptom | Cause | Solution |
|---------|-------|----------|
| API auth succeeds but `getAccountByAddr` returns `accountId: 0` | Signed requests work but no on-chain account | Create account with `createAccount()` |
| Can read order history but can't place orders | API works but no exchange account | Create account with `createAccount()` |
| 401 on signed requests | Bad/stale signature, clock skew, or revoked/expired key | Re-sign with a fresh timestamp + nonce; check key status |
| 403 on order placement | Key lacks `trade` scope | Enroll a `trade`-scoped key |
