# Authentication

Perpl uses wallet signature authentication with JWT cookies for session management.

## Overview

```
┌─────────┐     ┌─────────┐     ┌─────────┐
│  Client │────▶│ Payload │────▶│ Connect │
└─────────┘     └─────────┘     └─────────┘
     │               │               │
     │  1. Request   │               │
     │──────────────▶│               │
     │               │               │
     │  2. Signing   │               │
     │     Payload   │               │
     │◀──────────────│               │
     │               │               │
     │  3. Sign with wallet          │
     │               │               │
     │  4. Submit signature          │
     │──────────────────────────────▶│
     │               │               │
     │  5. JWT cookie + nonce        │
     │◀──────────────────────────────│
```

## Step 1: Request Signing Payload

**Endpoint**: `POST /api/v1/auth/payload`

**Request**:
```typescript
interface AuthPayloadRequest {
  chain_id: number;  // 10143 for Monad Testnet
  address: string;   // Wallet address (0x...)
}
```

**Response**:
```typescript
interface AuthPayloadResponse {
  message: string;    // SIWE message to sign
  nonce: string;      // Random nonce
  issued_at: number;  // Timestamp in milliseconds
  mac: string;        // Message authentication code
}
```

**Example**:
```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 10143;

const response = await fetch(`${API_URL}/v1/auth/payload`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address: '0x1234567890abcdef1234567890abcdef12345678'
  })
});

const payload = await response.json();
// payload.message contains SIWE message to sign
```

## Step 2: Sign the Payload

Sign the SIWE `message` using personal message signing.

**With viem**:
```typescript
import { signMessage } from 'viem/accounts';

const signature = await signMessage({
  message: payload.message,
  privateKey: '0x...'
});
```

**With ethers.js**:
```typescript
const signature = await wallet.signMessage(payload.message);
```

## Step 3: Submit Signature

**Endpoint**: `POST /api/v1/auth/connect`

**Request**:
```typescript
interface AuthConnectRequest {
  chain_id: number;
  address: string;
  message: string;       // From payload response
  nonce: string;         // From payload response
  issued_at: number;     // From payload response
  mac: string;           // From payload response
  signature: string;     // Your wallet signature
  ref_code?: string;     // Optional referral code
}
```

**Response**:
```typescript
interface AuthConnectResponse {
  nonce: string;  // Use this for authenticated requests
}
```

**Special Status Codes**:
| Code | Meaning | Action |
|------|---------|--------|
| 418 | Access code required | Include valid `ref_code` |
| 423 | Access code invalid/exhausted | Use different code |
| 403 | Access denied | Account blocked |

**Example**:
```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 10143;
const address = '0x1234567890abcdef1234567890abcdef12345678';

const authResponse = await fetch(`${API_URL}/v1/auth/connect`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address,
    message: payload.message,
    nonce: payload.nonce,
    issued_at: payload.issued_at,
    mac: payload.mac,
    signature: signature
  })
});

if (authResponse.status === 418) {
  console.log('Need access code');
}

const auth = await authResponse.json();
// auth.nonce is your session nonce
// JWT cookie is automatically set
```

## Using Authenticated Endpoints

After authentication, use these headers for authenticated REST requests:

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://testnet.perpl.xyz/api';

const headers = {
  'Content-Type': 'application/json',
  'X-Auth-Nonce': auth.nonce  // From auth/connect response
};

// JWT is sent automatically via cookies
const response = await fetch(`${API_URL}/v1/profile/ref-code`, {
  headers,
  credentials: 'include'  // Important: include cookies
});
```

## WebSocket Authentication

For the trading WebSocket, send `AuthSignIn` after connecting:

```typescript
const WS_URL = process.env.PERPL_WS_URL || 'wss://testnet.perpl.xyz';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 10143;

const ws = new WebSocket(`${WS_URL}/ws/v1/trading`);

ws.onopen = () => {
  ws.send(JSON.stringify({
    mt: 4,  // MsgTypeAuthSignIn
    nonce: auth.nonce,
    chain_id: CHAIN_ID
  }));
};
```

## Session Management

- JWT cookies have an expiration time (check `Set-Cookie` header)
- Re-authenticate when receiving 401/3401 errors
