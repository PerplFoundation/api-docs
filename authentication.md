# Authentication

Perpl authenticates programmatic clients (bots, terminals, scripts) with **API
keys**. An API key is an **Ed25519 key pair** — the server only ever stores the
public key, the private key never leaves your machine. There is no bearer token
or session cookie to leak: **every request is signed with the key's private
key**.

This page covers how to sign requests with a key you already have. To obtain a
key, see [Creating a key](#creating-a-key) below.

## Creating a key

- **Web UI** — connect your wallet and create a key at
  **mainnet** https://app.perpl.xyz/apikeys or
  **testnet** https://testnet.perpl.xyz/apikeys. The UI hands you the
  `X-API-Key` token and the private key.
- **Programmatically** — third-party integrations (e.g. trading terminals) can
  enroll keys directly. See **[Integrations](./integrations.md)** for the
  wallet-signed enrollment flow.

Either way you end up with two things, which the snippets below read from the
environment (as the [examples](./examples.md) do):

- `API_KEY` (from `PERPL_API_KEY`) — the opaque `X-API-Key` token.
- `privateKey` (from `PERPL_API_KEY_SECRET`, hex of the 32-byte key) — the Ed25519 private key.

A key carries a **scope** (`read`, `trade`, or both; trade implies read).
Withdrawals and transfers-out are never permitted via an API key. Scopes are
chosen at enrollment — see [Integrations → Scopes](./integrations.md#scopes).

## Authenticating REST requests

Sign a **canonical string** with the key and send four headers. No cookies.

The canonical string is these six fields joined by `\n` (newline):

```
<chain_id>
<HTTP_METHOD>          e.g. GET, POST
<request-target>       path + query string exactly as sent, e.g. /v1/trading/fills?count=100
<timestamp_ms>         unix epoch milliseconds, decimal
<nonce>                client-random, base64url (no padding)
<sha256(body) hex>     hex of SHA-256 over the raw request body ("" body → sha256 of empty string)
```

The signature is `base64url(ed25519_sign(privateKey, canonical))` (base64url, **no
padding**), sent in `X-API-Signature`.

| Header | Value |
|--------|-------|
| `X-API-Key` | the opaque token from enrollment |
| `X-API-Timestamp` | `timestamp_ms` used in the canonical string |
| `X-API-Nonce` | `nonce` used in the canonical string |
| `X-API-Signature` | `base64url(ed25519 signature)` |

```typescript
import { createHash, randomBytes } from 'crypto';
import * as ed from '@noble/ed25519';

const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;
const API_KEY = process.env.PERPL_API_KEY;
const privateKey = Buffer.from((process.env.PERPL_API_KEY_SECRET ?? '').replace(/^0x/, ''), 'hex');

async function signedRequest(method: string, target: string, body = '') {
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

// Example: read your recent fills
const res = await signedRequest('GET', '/v1/trading/fills?count=1');
console.log(await res.json());
```

> The `request-target` must match byte-for-byte what the server receives —
> include the query string (`?count=100&page=...`) exactly as sent.

## WebSocket authentication

For the trading WebSocket (`/ws/v1/trading`), authenticate by sending an
`ApiKeySignIn` frame (`mt: 29`) as the **first** message after the socket opens.
The signature covers the WS canonical string — four fields joined by `\n`:

```
<chain_id>
trading-ws-signin      literal action tag
<timestamp_ms>
<nonce>
```

```typescript
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;
const ts = Date.now().toString();
const nonce = randomBytes(16).toString('base64url');
const canonical = [CHAIN_ID, 'trading-ws-signin', ts, nonce].join('\n');
const sig = await ed.signAsync(Buffer.from(canonical), privateKey);

ws.onopen = () => {
  ws.send(JSON.stringify({
    mt: 29,                 // MsgTypeApiKeySignIn
    chain_id: CHAIN_ID,
    api_key: API_KEY,
    timestamp: ts,
    nonce,
    signature: Buffer.from(sig).toString('base64url'),
  }));
};
```

A `trade`-scoped key may place orders over the socket; a `read`-scoped key
receives snapshots/updates but order requests are rejected with close/status
`403`.

## Signature validity and errors

- **Timestamp window**: `X-API-Timestamp` must be within **30 seconds** of
  server time. Keep the client clock in sync.
- **Nonce**: single-use within the validity window — generate a fresh random
  `nonce` per request (replays are rejected).
- **Expiry / IP**: requests are rejected once the key is past `expires_at`, or
  when an `ip_cidrs` allow-list is set and the caller's IP is not covered.

| Status | Meaning | Action |
|--------|---------|--------|
| 401 | Missing/invalid headers, bad or stale signature, replayed nonce, revoked/expired key, IP not allowed | Re-sign with a fresh timestamp + nonce; check clock, key status and source IP |
| 403 | Scope insufficient (e.g. `read` key attempting to trade) | Enroll a `trade`-scoped key |
| WS 3401 | WebSocket authentication failure | Re-send a fresh signed `ApiKeySignIn` frame and reconnect |
