// Authenticating with an API key over the trading WebSocket.
//
// After the socket opens, the first frame must be a signed ApiKeySignIn (mt: 29).
// See authed_rest_requests.js for how the API key is loaded from the environment.
import * as ed from '@noble/ed25519';
import { randomBytes } from 'crypto';
import { fileURLToPath } from 'url';
import WebSocket from 'ws';

import { loadApiKey } from './authed_rest_requests.js';


const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

const PERPL_WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';
const TRADING_API = '/ws/v1/trading';

// MsgTypeApiKeySignIn: the API-key WebSocket sign-in frame type.
const API_KEY_SIGN_IN = 29;


async function signIn(ws, chainId, token, privateKey) {
    const timestamp = Date.now().toString();
    const nonce = randomBytes(16).toString('base64url');

    const canonical = [chainId, 'trading-ws-signin', timestamp, nonce].join('\n');
    const signature = Buffer.from(await ed.signAsync(Buffer.from(canonical), privateKey)).toString('base64url');

    const message = {
        mt: API_KEY_SIGN_IN,
        chain_id: chainId,
        api_key: token,
        timestamp,
        nonce,
        signature,
    };
    ws.send(JSON.stringify(message));

    ws.on('message', (data) => {
        console.log(`Received: ${data}`);
    });
}

function authedWebsocket(wsUrl, chainId, token, privateKey) {
    const url = wsUrl + TRADING_API;
    const ws = new WebSocket(url);

    ws.on('open', () => {
        signIn(ws, chainId, token, privateKey);
    });

    ws.on('error', (err) => {
        console.error('WebSocket error:', err);
    });
}

async function main() {
    const { token, privateKey } = loadApiKey();
    authedWebsocket(PERPL_WS_URL, CHAIN_ID, token, privateKey);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
