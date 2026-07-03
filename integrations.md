# Integrations (API Key Enrollment)

This page is for **third-party services** — trading terminals, bots, portfolio
trackers — that integrate with Perpl on behalf of a user's wallet. It covers how
to **enroll** an API key programmatically.

> End users can create a key themselves from the web UI
> (**mainnet** https://app.perpl.xyz/apikeys, **testnet** https://testnet.perpl.xyz/apikeys)
> and paste the token into your app. Implement the programmatic flow below when
> you want to enroll keys directly from your integration.

Once a key is enrolled, see **[Authentication](./authentication.md)** for how to
sign each request with it.

## How it works

An API key is an **Ed25519 key pair**. The server only ever stores the public
key; the private key never leaves the client. Enrollment is authorized once by
the user's wallet signature — after that, every request is signed with the key's
private key (there is no bearer token or session cookie).

```
┌──────────────┐                                    ┌──────────────┐
│ Integration  │  enroll (one-time, wallet-signed)  │    Perpl     │
│    client    │───────────────────────────────────▶│              │
│              │                                    │  stores the  │
│   Ed25519    │    opaque X-API-Key token          │  public key  │
│   keypair    │◀───────────────────────────────────│              │
└──────────────┘                                    └──────────────┘
```

1. **Generate** an Ed25519 key pair locally.
2. **Enroll** the public key: request an EIP-712 payload, sign it with the
   user's wallet (proves account ownership) **and** with the API key (proves you
   hold the private key), then submit both signatures. The server returns an
   opaque `X-API-Key` token.

### Scopes

A key is enrolled with a scope bitmask. Trade implies read. **Withdrawals and
transfers-out are never permitted via an API key, regardless of scope.**

| `scope_mask` | Name | Grants |
|--------------|------|--------|
| `1` | `read` | Read account, order, position, history, points/rewards data |
| `2` | `trade` | Place / cancel / modify orders (**implies read**) |
| `3` | `read \| trade` | Both |

### Delegated accounts

To enroll a key for a delegated account (an operator acting for another
profile), set `target_profile` to the delegated account address. `address`
remains the signing wallet (owner or operator); the server resolves and freezes
the principal at enrollment and validates the delegation on-chain.

### Endpoints

| Method | Path | Auth | Purpose |
|--------|------|------|---------|
| POST | `/api/v1/api-key/payload` | Wallet signature | Get the EIP-712 payload to sign |
| POST | `/api/v1/api-key/enroll` | Wallet signature | Enroll the key, receive the token |

> Listing and revoking keys is done from the web UI (`/apikeys`), not the API.

### Origin whitelisting

`/api-key/payload` and `/api-key/enroll` are CORS-enabled: they can be called
from your backend **or directly from client-side (browser) code**. In **both**
cases the request's `Origin` must be whitelisted by Perpl — enrollment records
the `Origin` it was created from (returned as `ApiKeyInfo.origin`), and requests
from a non-whitelisted `Origin` are rejected. Ask Perpl to whitelist the
origin(s) your integration enrolls from.

- **From a browser** the `Origin` header is set automatically and cannot be
  overridden — just ensure the page's origin is whitelisted.
- **From a server** (e.g. Node) set the `Origin` header explicitly on the
  request, as shown below.

---

## Step 1: Generate an Ed25519 key pair

The public key is sent as raw 32 bytes, `0x`-hex encoded.

**TypeScript / JavaScript** (`@noble/ed25519`):
```typescript
import * as ed from '@noble/ed25519';

const privateKey = ed.utils.randomPrivateKey();          // 32 bytes, keep secret
const publicKey  = await ed.getPublicKeyAsync(privateKey); // 32 bytes
const publicKeyHex = '0x' + Buffer.from(publicKey).toString('hex');
```

**Shell** (OpenSSL 3.x):
```bash
openssl genpkey -algorithm ed25519 -out apikey.pem
# raw 32-byte public key as 0x-hex:
PUBKEY_HEX=0x$(openssl pkey -in apikey.pem -pubout -outform DER | tail -c 32 | xxd -p -c 64)
```

## Step 2: Request the enrollment payload

**Endpoint**: `POST /api/v1/api-key/payload`

**Request** (`ApiKeyPayloadRequest`):
```typescript
interface ApiKeyPayloadRequest {
  chain_id: number;        // 143 mainnet, 10143 testnet
  address: string;         // signer wallet (owner or operator of the account)
  public_key: string;      // Ed25519 public key, 0x-hex (32 bytes)
  scope_mask: number;      // 1=read, 2=trade, 3=both
  label: string;           // human-readable key label (required)
  expires_at?: number;     // ms timestamp, 0 / omitted = never
  ip_cidrs?: string[];     // optional IP allow-list (max 4 CIDRs)
  target_profile?: string; // delegated account, if enrolling for one
}
```

**Response** (`ApiKeyPayloadResponse`):
```typescript
interface ApiKeyPayloadResponse {
  typed_data: any;  // EIP-712 typed data — sign this exactly as returned
  mac: string;      // opaque; echo back unchanged in the enroll request
}
```

```typescript
const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const ORIGIN = 'https://your-app.example';  // must be whitelisted by Perpl

const payloadRes = await fetch(`${API_URL}/v1/api-key/payload`, {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Origin': ORIGIN,  // set from a server; in a browser the Origin is set automatically
  },
  body: JSON.stringify({
    chain_id: 143,
    address: '0xUserWalletAddress',
    public_key: publicKeyHex,
    scope_mask: 3,
    label: 'my trading terminal',
  }),
});
const { typed_data, mac } = await payloadRes.json();
```

## Step 3: Sign and enroll

Enrollment requires **two** signatures over the returned `typed_data`:

1. **Wallet signature** — the wallet's secp256k1 EIP-712 signature. Proves the
   user owns (or operates) the account.
2. **Proof-of-possession** — an Ed25519 signature by the API private key over
   the EIP-712 digest `keccak256(0x1901 ‖ domainSeparator ‖ hashStruct(message))`.
   Proves you hold the private key for the public key being enrolled.

**TypeScript / JavaScript** (`ethers` v6 + `@noble/ed25519`):
```typescript
import { ethers } from 'ethers';
import * as ed from '@noble/ed25519';

// Illustrative only: an inline private key stands in for the signer here.
// Integrations are expected to sign with the user's CONNECTED wallet (e.g. a
// browser wallet via window.ethereum, or a wagmi/viem/ethers signer) — the
// private key never touches your code. Any EIP-712 signer works.
const wallet = new ethers.Wallet('0xUserWalletPrivateKey');

// ethers wants the EIP-712 types WITHOUT the EIP712Domain entry.
const { EIP712Domain, ...types } = typed_data.types;

// 1. Wallet secp256k1 EIP-712 signature (from the user's connected wallet).
const signature = await wallet.signTypedData(typed_data.domain, types, typed_data.message);

// 2. Ed25519 proof-of-possession over the EIP-712 digest.
const digest = ethers.TypedDataEncoder.hash(typed_data.domain, types, typed_data.message);
const popSig = await ed.signAsync(ethers.getBytes(digest), privateKey);
const popSignature = '0x' + Buffer.from(popSig).toString('hex');
```

**Endpoint**: `POST /api/v1/api-key/enroll`

**Request** (`ApiKeyEnrollRequest`):
```typescript
interface ApiKeyEnrollRequest {
  chain_id: number;
  address: string;
  typed_data: any;        // echoed from the payload response, unchanged
  mac: string;            // echoed from the payload response, unchanged
  signature: string;      // wallet EIP-712 signature, 0x-hex
  pop_signature: string;  // Ed25519 proof-of-possession, 0x-hex
  target_profile?: string;
}
```

**Response** (`ApiKeyEnrollResponse`): the enrolled key. `api_key.api_key` is the
opaque token you send as `X-API-Key` — **store it, it is not re-derivable**.

```typescript
interface ApiKeyInfo {
  api_key: string;       // opaque X-API-Key token
  address: string;
  scope_mask: number;
  label: string;
  ip_cidrs: string[];
  origin: string;        // HTTP Origin the key was enrolled from
  expires_at: number;    // ms, 0 = never
  last_used_at: number;  // ms, 0 = never
  created_at: number;    // ms
}
```

```typescript
const enrollRes = await fetch(`${API_URL}/v1/api-key/enroll`, {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Origin': ORIGIN,  // same whitelisted Origin as the payload request
  },
  body: JSON.stringify({
    chain_id: 143,
    address: '0xUserWalletAddress',
    typed_data,
    mac,
    signature,
    pop_signature: popSignature,
  }),
});
const { api_key } = await enrollRes.json();
const API_KEY = api_key.api_key; // the X-API-Key token — hand this to the request signer
```

**Enroll status codes**:
| Code | Meaning |
|------|---------|
| 404 | Target profile not found |
| 409 | Public key already registered (revoked keys can't be re-enrolled — use a fresh key pair) |
| 423 | Per-profile key limit reached (max 16 active keys) |

Listing and revoking keys is done from the web UI (`/apikeys`), not the API. A
revoked public key cannot be re-enrolled — generate a fresh key pair.

## Next steps

Once you hold an `X-API-Key` token and its Ed25519 private key, sign every
request as described in **[Authentication](./authentication.md)**. Runnable
enrollment + signing programs for JavaScript, TypeScript, Python and Rust are in
[`examples/`](./examples.md).
