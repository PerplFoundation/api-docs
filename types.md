# Type Reference

All types are derived from the backend API specification (Go → TypeScript via tygo).

## Primitive Types

```typescript
type ChainID = number;       // uint64 - EIP-155 chain ID
type InstanceID = number;    // uint32 - Protocol instance ID
type TokenID = number;       // uint32 - Token ID
type MarketID = number;      // uint32 - Market ID
type PerpetualID = number;   // uint32 - Perpetual ID (smart contract)
type FeeLevelID = number;    // uint32 - Fee level ID
type AccountID = number;     // uint64 - Trading account ID
type OrderID = number;       // uint64 - Order ID
type RequestID = number;     // uint64 - Request ID (idempotency key)
type PositionID = number;    // uint64 - Position ID
type Decimals = number;      // uint8 - Decimal places
type Fraction = number;      // uint32 - Fraction in hundredths
type Micros = number;        // int64 - Value in 10^-6 fractions
type Amount = string;        // Decimal string for large numbers
type Price = number;         // uint64 - Scaled price
type SPrice = number;        // int64 - Signed scaled price
type Size = number;          // uint64 - Scaled size
```

## Scaling

Prices and sizes are scaled integers. Use market config for decimals:

```typescript
// Convert scaled price to human readable
function scalePrice(scaled: number, priceDecimals: number): number {
  return scaled / Math.pow(10, priceDecimals);
}

// Convert human price to scaled
function unscalePrice(price: number, priceDecimals: number): number {
  return Math.round(price * Math.pow(10, priceDecimals));
}

// Example: BTC price with 1 decimal
// scaled: 950000 → human: $95,000.0
// human: $95,000.0 → scaled: 950000
```

---

## Timestamps

### BlockTimestamp

```typescript
interface BlockTimestamp {
  b?: number;  // Block number
  t?: number;  // Timestamp (milliseconds)
}
```

### BlockTxTimestamp

```typescript
interface BlockTxTimestamp {
  b?: number;    // Block number
  t?: number;    // Timestamp (ms)
  tx?: number;   // Transaction index in block
  txid?: string; // Transaction hash
}
```

### BlockTxLogTimestamp

```typescript
interface BlockTxLogTimestamp {
  b?: number;    // Block number
  t?: number;    // Timestamp (ms)
  tx?: number;   // Transaction index
  txid?: string; // Transaction hash
  l?: number;    // Log index in transaction
}
```

---

## Chain & Protocol

### Chain

```typescript
interface Chain {
  ver: number;
  chain_id: ChainID;
  name?: string;
  icons?: string[];
  native_token?: Token;
  rpc_urls?: string[];
  block_explorer_urls?: string[];
  gas: GasPrice;
}
```

### Token

```typescript
interface Token {
  ver: number;
  id?: TokenID;
  address?: string;       // ERC-20 address (empty for native)
  symbol: string;
  name: string;
  icon?: string;
  decimals: Decimals;
  display_precision: Decimals;
  usd_index?: string;
}
```

### ProtocolInstance

```typescript
interface ProtocolInstance {
  ver: number;
  id: InstanceID;
  address: string;                    // Exchange contract
  collateral_token_id: TokenID;
  max_account_equity?: Amount;
  max_account_trigger_orders: number;
}
```

### GasPrice

```typescript
interface GasPrice {
  at: BlockTimestamp;
  h: number;      // Head block
  max: Amount;    // Maximum priority
  p95: Amount;    // 95th percentile
  p50: Amount;    // 50th percentile
  min: Amount;    // Minimum priority
  base: Amount;   // Base fee only
}
```

---

## Market

### Market

```typescript
interface Market {
  ver: number;
  id: MarketID;
  instance_id: InstanceID;
  perpetual_id: PerpetualID;
  symbol: string;
  name: string;
  size_units: string;
  icon: string;
  funding_interval_sec: number;
  funding_interval_blocks: number;
  order_ttl_blocks: number;
  order_retry_blocks: number;
  order_max_price_impact_percent: number;
  config: MarketConfig;
  state: MarketState;
  funding: FundingEvent;
}
```

### MarketConfig

```typescript
interface MarketConfig {
  at: BlockTimestamp;
  is_open: boolean;
  price_decimals: Decimals;
  size_decimals: Decimals;
  min_account_open_amount: Amount;
  min_posting_amount: Amount;
  min_settle_amount: Amount;
  initial_margin: Fraction;       // e.g., 1000 = 10% (10x max)
  maintenance_margin: Fraction;   // e.g., 2000 = 5%
  maker_fee: Micros;              // Negative = rebate
  taker_fee: Micros;
  recycle_fee: Amount;
}
```

### MarketState

```typescript
interface MarketState {
  at: BlockTimestamp;
  orl: Price;   // Oracle price
  mrk: Price;   // Mark price
  lst: Price;   // Last trade price
  mid: Price;   // Mid price
  bid: Price;   // Best bid
  ask: Price;   // Best ask
  prv: Price;   // Price 24h ago
  dv: Size;     // Daily volume (size)
  dva: Amount;  // Daily volume (amount)
  oi: Size;     // Open interest
  tvl: Amount;  // Total value locked
}
```

### FundingEvent

```typescript
interface FundingEvent {
  at: BlockTimestamp;
  feb: number;    // Funding event block
  rate: Micros;   // Funding rate (10^-6)
  idx: Price;     // Index price
  ppl: SPrice;    // Payment per lot
  sum: SPrice;    // Funding sum
}
```

---

## Order

### Order

```typescript
interface Order {
  at: BlockTxLogTimestamp;  // Update timestamp
  c: BlockTxTimestamp;      // Creation timestamp
  rq: RequestID;
  mkt: MarketID;
  acc: AccountID;
  oid: OrderID;
  scid: OrderID;            // Smart contract order ID
  st: OrderStatus;
  sr: OrderStatusReason;
  t: OrderType;
  r?: boolean;              // Remove from open orders
  p?: Price;                // Limit price (0 = market)
  os: Size;                 // Original size
  fp: Price;                // Fill price (weighted avg)
  fs: Size;                 // Filled size
  f: Amount;                // Fee paid
  tif?: number;             // Time-in-force block
  fl: OrderFlags;
  tp?: Price;               // Trigger price
  tpc?: TriggerPriceCondition;
  lp?: PositionID;          // Linked position
  mm: number;               // Max matches
  lv: number;               // Leverage (hundredths)
}
```

### OrderType

| Value | Name | Description |
|-------|------|-------------|
| 0 | Unspecified | |
| 1 | OpenLong | Open long position |
| 2 | OpenShort | Open short position |
| 3 | CloseLong | Close long position |
| 4 | CloseShort | Close short position |
| 5 | Cancel | Cancel order |
| 6 | IncreasePositionCollateral | Add margin |
| 7 | Change | Modify order |

### OrderStatus

| Value | Name |
|-------|------|
| 0 | Unspecified |
| 1 | Pending |
| 2 | Open |
| 3 | PartiallyFilled |
| 4 | Filled |
| 5 | Canceled |
| 6 | Expired |
| 7 | Failed |
| 8 | Untriggered |
| 9 | Triggered |

### OrderFlags

| Value | Name | Description |
|-------|------|-------------|
| 0 | GoodTillCancel | Default, stays until filled/canceled |
| 1 | PostOnly | Only maker, rejects if would take |
| 2 | FillOrKill | Fill entire order or cancel |
| 4 | ImmediateOrCancel | Fill what's available, cancel rest |

### TriggerPriceCondition

| Value | Name | Description |
|-------|------|-------------|
| 0 | Unspecified | |
| 1 | GTE | Trigger when price >= trigger |
| 2 | LTE | Trigger when price <= trigger |

### OrderStatusReason

Common values:

| Value | Name |
|-------|------|
| 16 | ImmediateOrCancelExecuted |
| 22 | MakerOrderFilled |
| 28 | OrderCancelled |
| 35 | OrderPlaced |
| 43 | TakerOrderFilled |
| 46 | UnmatchedLotRemainsInFillOrKill |

See spec.ts for full list (56 values).

---

## Fill

```typescript
interface Fill {
  at: BlockTxLogTimestamp;
  mkt: MarketID;
  acc: AccountID;
  oid: OrderID;
  t: OrderType;
  l: LiquiditySide;   // 1=Maker, 2=Taker
  p?: Price;          // Fill price
  s: Size;            // Filled size
  f: Amount;          // Fee (negative = rebate)
}
```

### LiquiditySide

| Value | Name |
|-------|------|
| 0 | Unspecified |
| 1 | Maker |
| 2 | Taker |

---

## Position

```typescript
interface Position {
  at: BlockTxLogTimestamp;
  mkt: MarketID;
  acc: AccountID;
  pid: PositionID;
  rq: RequestID;
  oid: OrderID;
  st: PositionStatus;
  sr: PositionStatusReason;
  sd: PositionType;    // 1=Long, 2=Short
  c: Amount;           // Collateral
  ep: Price;           // Entry price
  s: Size;             // Size
  fee: Amount;         // Fees paid
  efs: SPrice;         // Entry funding sum
  lv: number;          // Leverage (hundredths)
  dpnl?: Amount;       // Realized delta PnL
  fnd?: Amount;        // Realized funding PnL
  xp?: Price;          // Exit price
  xfs: SPrice;         // Exit funding sum
  ots: BlockTxTimestamp; // Open timestamp
  e: Position[];       // Settlement events
}
```

### PositionType

| Value | Name |
|-------|------|
| 0 | Unspecified |
| 1 | Long |
| 2 | Short |

### PositionStatus

| Value | Name |
|-------|------|
| 0 | Unspecified |
| 1 | Open |
| 2 | Closed |
| 3 | Liquidated |
| 4 | Deleveraged |
| 5 | Unwound |
| 6 | Failed |

---

## Account & Wallet

### Wallet

```typescript
interface Wallet {
  mt: number;
  at: BlockTimestamp;
  addr: string;           // Wallet address
  n: number;              // Current nonce
  fl: FeeLevelID;
  as?: Account[];         // Accounts (snapshot only)
}
```

### Account

```typescript
interface Account {
  mt: number;
  in: InstanceID;
  id: AccountID;
  fr: boolean;      // Is frozen
  fw: boolean;      // Allows forwarding
  b: Amount;        // Balance
  lb: Amount;       // Locked balance
  h?: AccountEvent[];
}
```

### AccountEvent

```typescript
interface AccountEvent {
  at: BlockTxLogTimestamp;
  in: InstanceID;
  id: AccountID;
  et: AccountEventType;
  m?: MarketID;
  r?: OrderID;      // Request ID
  o?: OrderID;
  p?: PositionID;
  a: Amount;        // Amount change
  b: Amount;        // Updated balance
  lb: Amount;       // Locked balance
  f: Amount;        // Fee
}
```

### AccountEventType

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

## Market Data

### L2PriceLevel

```typescript
interface L2PriceLevel {
  p: Price;   // Price (scaled)
  s: Size;    // Size (scaled)
  o: number;  // Number of orders
}
```

### Trade

```typescript
interface Trade {
  at: BlockTxLogTimestamp;
  p: Price;
  s: Size;
  sd: TradeSide;  // 1=Buy, 2=Sell
}
```

### Candle

```typescript
interface Candle {
  t: number;    // Open timestamp (ms)
  o: Price;     // Open
  c: Price;     // Close
  h: Price;     // High
  l: Price;     // Low
  v: Amount;    // Volume
  n: number;    // Trade count
}
```

---

## Profile

### RefCode

```typescript
interface RefCode {
  code: string;
  limit: number;
  used: number;
}
```

### ContactInfo

```typescript
interface ContactInfo {
  contact: string;
  x_challenge: string;
}
```

### Announcement

```typescript
interface Announcement {
  id: number;
  title: string;
  content: string;
}
```
