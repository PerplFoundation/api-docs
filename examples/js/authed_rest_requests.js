// Authenticating with the API (Ed25519 API key) and making a signed REST request.
//
// An API key is an Ed25519 key pair. The server only stores the public key;
// the private key never leaves this machine. Every request is signed with the
// key's private key.
//
// This example uses a key you already have. Create one in the web UI
// (connect wallet -> create key):
//   Mainnet: https://app.perpl.xyz/apikeys
//   Testnet: https://testnet.perpl.xyz/apikeys
// then set PERPL_API_KEY (the X-API-Key token) and PERPL_API_KEY_SECRET (hex of
// the 32-byte Ed25519 private key). To enroll a key programmatically instead,
// see enroll_api_key.js.
import * as ed from '@noble/ed25519';
import { createHash, randomBytes } from 'crypto';
import { fileURLToPath } from 'url';


const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;
const API_KEY = process.env.PERPL_API_KEY;
const API_KEY_SECRET = process.env.PERPL_API_KEY_SECRET;

const TEST_API = '/v1/trading/fills?count=1';


// Load an already-enrolled API key from the environment. Returns { token, privateKey }
// where token is the opaque X-API-Key value and privateKey is the 32-byte Ed25519 secret.
export function loadApiKey() {
    if (!API_KEY || !API_KEY_SECRET) {
        console.error(
            'Set PERPL_API_KEY and PERPL_API_KEY_SECRET — create a key at the web UI ' +
            'https://app.perpl.xyz/apikeys (testnet https://testnet.perpl.xyz/apikeys) ' +
            'or run `node enroll_api_key.js`',
        );
        process.exit(1);
    }
    const privateKey = Uint8Array.from(Buffer.from(API_KEY_SECRET.replace(/^0x/, ''), 'hex'));
    return { token: API_KEY, privateKey };
}


// Sign a request with the API key and send it. `target` is the path+query exactly
// as the gateway receives it (e.g. '/v1/trading/fills?count=100'); it is signed
// byte-for-byte and must match the URL sent. `body` is the raw JSON string ('' for GET).
export async function signedFetch(method, target, body = '') {
    const { token, privateKey } = loadApiKey();
    const timestamp = Date.now().toString();
    const nonce = randomBytes(16).toString('base64url');
    const bodyHash = createHash('sha256').update(body).digest('hex');

    const canonical = [CHAIN_ID, method, target, timestamp, nonce, bodyHash].join('\n');
    const sig = Buffer.from(await ed.signAsync(Buffer.from(canonical), privateKey)).toString('base64url');

    return fetch(`${API_URL}${target}`, {
        method,
        headers: {
            'X-API-Key': token,
            'X-API-Timestamp': timestamp,
            'X-API-Nonce': nonce,
            'X-API-Signature': sig,
            ...(body ? { 'Content-Type': 'application/json' } : {}),
        },
        ...(body ? { body } : {}),
    });
}


async function main() {
    const res = await signedFetch('GET', TEST_API);
    const data = await res.json();
    console.log(`Authed Request: ${API_URL}${TEST_API}`);
    console.log('Authed Response:', data);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
