// Streaming from the public websocket API without authentication
import WebSocket from 'ws';


const API_URL = process.env.PERPL_WS_URL || 'wss://app.perpl.xyz';
const WS_URL = '/ws/v1/market-data';

const BTC_MAINNET = 1;   // 16 for testnet
const PERPL_CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;   // 10143 for testnet
const HOUR = 3600;

const MsgTypeSubscriptionRequest = 5;


interface Subscription {
    stream: string;
    subscribe: boolean;
}

interface SubscriptionMessage {
    mt: number;
    subs: Subscription[];
}


function websocketConnect(): void {
    const url = API_URL + WS_URL;
    const ws = new WebSocket(url);

    ws.on('open', () => {
        const message: SubscriptionMessage = {
            mt: MsgTypeSubscriptionRequest,
            subs: [
                { stream: `heartbeat@${PERPL_CHAIN_ID}`, subscribe: true },
                { stream: `order-book@${BTC_MAINNET}`, subscribe: true },   // BTC order book (mainnet)
                { stream: `trades@${BTC_MAINNET}`, subscribe: true },        // BTC trades (mainnet)
                { stream: `candles@${BTC_MAINNET}*${HOUR}`, subscribe: true }, // BTC 1h candles (mainnet)
            ],
        };
        ws.send(JSON.stringify(message));
    });

    ws.on('message', (data: WebSocket.RawData) => {
        console.log(`Received: ${data}`);
    });

    ws.on('error', (err: Error) => {
        console.error('WebSocket error:', err);
    });
}

websocketConnect();
