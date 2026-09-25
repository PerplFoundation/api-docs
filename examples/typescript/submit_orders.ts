// Submitting orders over HTTP (POST /v1/trading/orders).
//
// The order submission endpoint takes a BATCH of orders and answers with one
// status per order, at the position of the order it answers. This example walks
// the whole flow a batch client needs:
//
//   1. read the wallet snapshot for the account, the head block and the last
//      request ID the account accepted,
//   2. submit a batch containing one valid order and one deliberately invalid
//      one, to show that a batch is not a transaction,
//   3. wait a block and read the open orders back, because a zero status code
//      acknowledges forwarding, not execution,
//   4. cancel the order that was placed.
//
// Requires an API key with the `trade` scope, and an account with order
// forwarding enabled (see README.md#enabling-order-forwarding-one-click-trading).
//
// This program places a REAL order. It is a post-only bid far below the mid
// price, so it should rest rather than fill, but it locks collateral until it is
// cancelled. Set PERPL_SUBMIT=1 to actually send it; without that it prints the
// batch it would have sent and exits.
import { fileURLToPath } from 'url';
import { signedFetch } from './authed_rest_requests.js';


const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const MARKET_ID = Number(process.env.PERPL_MARKET_ID) || 1;
const ORDER_SIZE = Number(process.env.PERPL_ORDER_SIZE) || 0.001;
const SUBMIT = process.env.PERPL_SUBMIT === '1';

// Order types (see types.md#ordertype)
const ORDER_TYPE_OPEN_LONG = 1;
const ORDER_TYPE_CANCEL = 5;
// Order flags (see types.md#orderflags)
const ORDER_FLAG_POST_ONLY = 1;


// One order of a batch — an OrderSpec (see types.md#orderspec). The same shape
// the WebSocket transport carries under its message header.
interface OrderSpec {
    rq: number;
    mkt: number;
    acc: number;
    oid?: number;
    t: number;
    p?: number;
    s?: number;
    fl: number;
    lv: number;
    lb: number;
}

interface Status {
    code: number;
    error?: string;
}

interface BatchStatusResponse {
    mt: number;
    cid?: number;
    status: Status;
    statuses?: Status[];
}


async function getJSON<T>(target: string): Promise<T> {
    const res = await signedFetch('GET', target);
    if (!res.ok) throw new Error(`GET ${target} -> ${res.status} ${await res.text()}`);
    return res.json() as Promise<T>;
}


// The market's configuration, for the decimals and the `lb` ceiling.
async function getMarket(marketId: number): Promise<any> {
    const ctx = await (await fetch(`${API_URL}/v1/pub/context`)).json();
    const market = ctx.markets.find((m: any) => m.id === marketId);
    if (!market) throw new Error(`market ${marketId} not found in /v1/pub/context`);
    return market;
}


// Mid price of the market, scaled by its price decimals. The ticker is keyed by
// market ID the way the WebSocket stream keys it, so a single-market response is
// a map with one entry.
async function getMidPrice(marketId: number): Promise<number> {
    const ticker = await (await fetch(`${API_URL}/v1/market-data/${marketId}/ticker`)).json();
    const state = ticker.d[marketId];
    if (!state) throw new Error(`no state for market ${marketId}`);
    return state.mid;
}


// Submit a batch and report each order's own status.
//
// The HTTP status is 200 whenever the orders were judged at all, however many of
// them were refused — so the per-order statuses are what has to be read. A
// non-zero top-level `status` means the request was refused before any order was
// looked at, and `statuses` is then empty.
async function submitBatch(orders: OrderSpec[]): Promise<Status[]> {
    const body = JSON.stringify({ d: orders });
    const res = await signedFetch('POST', '/v1/trading/orders', body);
    const reply: BatchStatusResponse = await res.json();

    if (reply.status?.code) {
        // Refused as a whole: nothing was forwarded. The code is repeated as the
        // HTTP status, so either can be read.
        console.log(`batch refused (${res.status}): ${reply.status.code} ${reply.status.error}`);
        return [];
    }

    const statuses = reply.statuses ?? [];
    statuses.forEach((status, i) => {
        const order = orders[i];  // orders are identified by position, not by an echo
        if (status.code === 0) {
            console.log(`  [${i}] rq=${order.rq} accepted for forwarding`);
        } else {
            console.log(`  [${i}] rq=${order.rq} refused: ${status.code} ${status.error}`);
        }
    });
    return statuses;
}


// A zero status code acknowledges forwarding, not execution: the outcome is
// settled a block later. Poll the open orders until the request ID shows up.
async function awaitOrder(requestId: number, attempts = 10): Promise<any | null> {
    for (let i = 0; i < attempts; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        const snapshot = await getJSON<{ d: any[] }>('/v1/trading/orders');
        const order = snapshot.d.find((o) => o.rq === requestId);
        if (order) return order;
    }
    return null;
}


async function main(): Promise<void> {
    // 1. The wallet snapshot carries the account, whether it may forward orders,
    //    the last request ID it accepted, and — as `sn` — the block the trading
    //    state is current at. An HTTP client has no heartbeat stream, so this is
    //    where the head block comes from.
    const wallet = await getJSON<any>('/v1/trading/wallet');
    const account = wallet.as?.[0];
    if (!account) throw new Error('wallet holds no exchange account');
    if (!account.fw) {
        throw new Error(
            `account ${account.id} may not forward orders — call allowOrderForwarding(true) ` +
            'on the Exchange contract, see README.md#enabling-order-forwarding-one-click-trading',
        );
    }

    const headBlock: number = wallet.sn;
    // Request IDs must not decrease per account. Seed from `lfr`, the last
    // request ID the exchange accepted for this account.
    let nextRequestId: number = account.lfr + 1;

    const market = await getMarket(MARKET_ID);
    const mid = await getMidPrice(MARKET_ID);

    // A post-only bid at half the mid price: it cannot cross, so it rests.
    const price = Math.round(mid * 0.5);
    const size = Math.round(ORDER_SIZE * Math.pow(10, market.config.size_decimals));
    // head < lb <= head + order_ttl_blocks
    const lastExecBlock = headBlock + Math.min(30, market.order_ttl_blocks);

    const placeRequestId = nextRequestId++;
    const batch: OrderSpec[] = [
        {
            rq: placeRequestId,
            mkt: MARKET_ID,
            acc: account.id,
            t: ORDER_TYPE_OPEN_LONG,
            p: price,
            s: size,
            fl: ORDER_FLAG_POST_ONLY,
            lv: 100,              // 1x, in hundredths
            lb: lastExecBlock,
        },
        {
            // Deliberately invalid: an unknown market. It is refused on its own
            // and the order above still stands — a batch is not a transaction.
            rq: nextRequestId++,
            mkt: 999999,
            acc: account.id,
            t: ORDER_TYPE_OPEN_LONG,
            p: price,
            s: size,
            fl: 0,
            lv: 100,
            lb: lastExecBlock,
        },
    ];

    console.log(`account ${account.id}, head block ${headBlock}, mid ${mid}`);
    if (!SUBMIT) {
        console.log('PERPL_SUBMIT is not 1 — printing the batch instead of sending it:');
        console.log(JSON.stringify({ d: batch }, null, 2));
        return;
    }

    console.log('submitting batch:');
    const statuses = await submitBatch(batch);
    if (statuses[0]?.code !== 0) return;

    // 3. Read the outcome back. The acknowledgement above said the order was
    //    forwarded; whether it posted is settled a block later.
    const order = await awaitOrder(placeRequestId);
    if (!order) {
        console.log('order did not appear in the snapshot — check the order history for a failure');
        return;
    }
    console.log(`order ${order.oid} is open at ${order.p} (status ${order.st})`);

    // 4. Cancel it. One order is a batch of one.
    console.log('cancelling:');
    await submitBatch([
        {
            rq: nextRequestId++,
            mkt: MARKET_ID,
            acc: account.id,
            oid: order.oid,
            t: ORDER_TYPE_CANCEL,
            fl: 0,
            lv: 0,
            lb: 0,
        },
    ]);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
