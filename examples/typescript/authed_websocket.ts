// Authenticating with the API and streaming from the trades API
import { randomUUID } from 'crypto';
import { fileURLToPath } from 'url';
import WebSocket from 'ws';

import { perplAuth } from './authed_rest_requests.js';


const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const PERPL_CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;

const PERPL_WS_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';
const TRADING_API = '/ws/v1/trading';

const AUTH_SIGN_IN = 4;


const WALLET_ADDRESS = '0xYourWalletAddress';
const WALLET_KEY = '0xYourWalletPrivateKey';


interface AuthSignInMessage {
    mt: number;
    chain_id: number;
    nonce: string;
    ses: string;
}


function startStream(ws: WebSocket, nonce: string): void {
    const ses = randomUUID();
    const message: AuthSignInMessage = {
        mt: AUTH_SIGN_IN,
        chain_id: PERPL_CHAIN_ID,
        nonce,
        ses,
    };
    ws.send(JSON.stringify(message));

    ws.on('message', (data: WebSocket.RawData) => {
        console.log(`Received: ${data}`);
    });
}

function authedWebsocket(wsUrl: string, nonce: string, authTokenCookie: string | null): void {
    const url = wsUrl + TRADING_API;
    const ws = new WebSocket(url, {
        headers: {
            'X-Auth-Nonce': nonce,
            'Cookie': `auth-token=${authTokenCookie}`,
        },
    });

    ws.on('open', () => {
        startStream(ws, nonce);
    });

    ws.on('error', (err: Error) => {
        console.error('WebSocket error:', err);
    });
}

async function main(): Promise<void> {
    const { nonce, authTokenCookie } = await perplAuth(API_URL, PERPL_CHAIN_ID, WALLET_ADDRESS, WALLET_KEY);
    authedWebsocket(PERPL_WS_URL, nonce, authTokenCookie);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
