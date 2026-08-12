# Integrations (API Key Enrollment)

This page is for **third-party services** — trading terminals, bots, portfolio
trackers — that integrate with Perpl on behalf of a user's wallet. It covers how
to **enroll** an API key programmatically, and — for integrations that charge
their own fee on the flow they route — how to use
**[builder codes](#builder-codes)**.

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

  // Builder codes only — see the Builder codes section below.
  builder_id?: number;               // your registered builder code, 1..255
  max_builder_fee_per_100k?: number; // fee ceiling the user authorizes, 1 = 0.1 bps
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

const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

// API key scope bitmask: 1 = read, 2 = trade (implies read), 3 = both.
const SCOPE_MASK = 3;

const payloadRes = await fetch(`${API_URL}/v1/api-key/payload`, {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'Origin': ORIGIN,  // set from a server; in a browser the Origin is set automatically
  },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address: '0xUserWalletAddress',
    public_key: publicKeyHex,
    scope_mask: SCOPE_MASK,
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

  // Builder terms, present only on a builder-bound key.
  builder_id?: number;                // the code the key submits under
  builder_name?: string;              // registered display name
  max_builder_fee_per_100k?: number;  // enrolled ceiling, 1 = 0.1 bps
  max_builder_fee_pct?: string;       // the same ceiling formatted, e.g. "0.100%"
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
    chain_id: CHAIN_ID,
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

---

## Builder codes

A **builder code** identifies your integration to Perpl. It lets you charge your
own fee on the orders you route — on top of the protocol fee, attributed to your
code and settled to you — and gives you volume attribution even when you charge
nothing. Builder codes are optional: everything above works without one.

Three things have to line up:

1. Perpl registers your builder code — [Registering as a builder](#registering-as-a-builder).
2. Each user's key is enrolled **bound to that code**, with a fee ceiling the
   user signs for — [Enrolling a builder-bound key](#enrolling-a-builder-bound-key).
3. Each order states the fee it wants to charge, within that ceiling —
   [Charging a builder fee](#charging-a-builder-fee).

### Fee units

Builder fees are expressed in **hundred-thousandths** (`per_100k`), the unit the
on-chain fee schedule uses. `1` = 0.1 bps = 0.001%.

> Not the same unit as the market fee rates on the API, which are in **micros**
> (`10^-6`) — see [Fees & fee tiers](./README.md#fees--fee-tiers). `1 per_100k` =
> `10 micros`.

| `per_100k` | bps | percent |
|---|---|---|
| `1` | 0.1 | 0.001% |
| `10` | 1 | 0.01% |
| `100` | 10 | 0.1% |
| `1000` | 100 | 1% |

The maximum is `100` (0.1%).

### Registering as a builder

**Contact Perpl to register.** Builder codes are issued by the operator; there is
no self-service endpoint. You provide:

| | |
|---|---|
| **Display name** | Shown to *your users* in the wallet prompt when they authorize a key (see below) |
| **Perpl account address** | The address of your Perpl account your accrued builder fees are paid out to. |

You receive a **builder id** in `1..255` (the id is a `uint8` on-chain, so the
registry is deliberately small).

### Enrolling a builder-bound key

Same two-step flow as above — the only difference is two extra fields on the
**payload** request. `builder_id` cannot be attached to an already-enrolled key;
it is frozen at enrollment and covered by the user's signature.

```typescript
const BUILDER_ID = Number(process.env.PERPL_BUILDER_ID) || 0;
const MAX_BUILDER_FEE = Number(process.env.PERPL_MAX_BUILDER_FEE_PER_100K) || 0;

const payloadRes = await fetch(`${API_URL}/v1/api-key/payload`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', 'Origin': ORIGIN },
  body: JSON.stringify({
    chain_id: CHAIN_ID,
    address: '0xUserWalletAddress',
    public_key: publicKeyHex,
    scope_mask: SCOPE_MASK,
    label: 'my trading terminal',
    builder_id: BUILDER_ID,                     // your registered code
    max_builder_fee_per_100k: MAX_BUILDER_FEE,  // ceiling: 100 (0.1%) per order
  }),
});
```

All failures are `400`:

| Condition | Message |
|---|---|
| `builder_id` outside `1..255` | `builder_id must be in 1..255` |
| `max_builder_fee_per_100k` above the environment ceiling | `max_builder_fee_per_100k must be at most <N>` |
| `max_builder_fee_per_100k` without a `builder_id` | `max_builder_fee_per_100k requires a builder_id` |
| Non-zero fee ceiling on a `read`-only key | `max_builder_fee_per_100k requires the trade scope` |
| Code not registered / not enabled | `builder code <N> is not registered` |
| Builder enrollment not enabled in that environment | `builder-bound api keys are not enabled` |

`max_builder_fee_per_100k: 0` with a `builder_id` is valid and useful:
**attribution without a fee**. It is also the only shape a `read`-scoped builder
key can take — such a key can never place an order.

#### What the user signs

**The signer is the end user, not you.** You generate the Ed25519 key pair and
supply the proof-of-possession; the user's wallet signs the EIP-712 payload. That
signature *is* the fee authorization — there is no separate builder-side approval
step — so the payload is written to be legible in the wallet prompt:

- the machine-enforced terms are EIP-712 fields (`builderId`,
  `maxBuilderFeePer100K`), and
- the same terms in prose are in the payload's `statement` field, naming your
  registered builder name and the ceiling as a percentage. This is what the user
  actually reads before approving.

`/api-key/enroll` **re-derives that statement** from the registry and rejects the
enrollment if it does not match the one signed. The practical consequence: if
your builder name changes between the payload and the enroll call, the enrollment
fails — request a fresh payload and have the user sign again.

The enroll response echoes the terms back (`builder_id`, `builder_name`,
`max_builder_fee_per_100k`, `max_builder_fee_pct`) so you can confirm you
registered what you intended. Users see the same fields for every key on their
`/apikeys` page and can revoke any key there at any time — consent is granted
once, visibility and revocation are continuous.

### Charging a builder fee

Orders are placed over the trading WebSocket (`mt: 22`, see
[WebSocket → Placing Orders](./websocket.md#placing-orders)). A builder-bound key
adds one field:

```typescript
ws.send(JSON.stringify({
  mt: 22,
  sn: 1,
  rq: Date.now(),      // idempotency key, strictly increasing per account
  mkt: 1, acc: accountId,   // BTC (mainnet)
  t: 1,                // OpenLong
  p: 0, s: 10000,      // market order, 0.1 BTC
  fl: 0, lv: 1000,     // GTC, 10x leverage
  lb: head + market.order_ttl_blocks,
  bf: 50,              // builder fee: 5 bps, must be <= the key's ceiling
}));
```

- **There is no `builder_id` on the request.** The code comes from the
  authenticating key. A client that could name its own code could attribute
  another builder's flow to itself.
- **The enrolled ceiling is a maximum, not a default.** Set `bf` on every order
  you want to charge for. Omitting it is *not* an error: the order executes,
  attributed to your code, at zero fee — you simply earn nothing on it.
- **A fee above the ceiling is rejected, not clamped.** Silently reducing it
  would make your accounting disagree with the chain.
- The fee applies to the size that **opens or increases** a position. Closing or
  reducing fills carry no builder fee.
- Builder fees only exist on orders routed through the API. Orders a user sends
  directly on-chain cannot be attributed to a builder.

Rejections arrive as the usual `mt: 3` status, and no `mt: 24` update follows:

| `error` | Condition | `code` |
|---|---|---|
| `builder fee not permitted for this api key` | `bf` above the key's ceiling, or any `bf` on a non-builder key | 400 |
| `api key lacks trade scope` | `read`-scoped key | 403 |

### Reconciling what was charged

Fees are reported **gross**: the `f` (fee) amount on orders, fills and account
events is the total the user paid, protocol fee **plus** builder fee. The builder
portion is broken out alongside it, so never add the two together.

| Field | On | Meaning |
|---|---|---|
| `bfa` | `Fill` (`mt: 25`), `Order` (`mt: 24`), `AccountEvent` | Builder-fee portion of that event's `f` |
| `tbf` | `AccountStats` | Lifetime builder fees inside `tf` (total fees) |

Both are omitted when zero, so an ordinary (non-builder) account sees no change.
A mis-integration is visible within one fill: flow that reaches you with `bf`
unset shows up as volume with `bfa: 0`.

### Getting paid

Accrued builder fees are collected per builder code and settled to the Perpl account
registered with your code, on a periodic epoch schedule.

---

## Next steps

Once you hold an `X-API-Key` token and its Ed25519 private key, sign every
request as described in **[Authentication](./authentication.md)**. Runnable
enrollment + signing programs for JavaScript, TypeScript, Python and Rust are in
[`examples/`](./examples.md).
