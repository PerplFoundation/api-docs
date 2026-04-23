// Authenticating with the API and making a REST request
import { ethers } from 'ethers';
import { fileURLToPath } from 'url';


const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const AUTH_PAYLOAD_URL = '/v1/auth/payload';
const CONNECT_PAYLOAD_URL = '/v1/auth/connect';
const PERPL_CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;
const TEST_API = '/v1/profile/contact-info';


const WALLET_ADDRESS = '0xYourWalletAddress';
const WALLET_KEY = '0xYourWalletPrivateKey';


export async function perplAuth(apiUrl, chainId, walletAddress, walletKey, ref_code='') {
    // Step 1: Get signing payload
    const authPayload = { chain_id: chainId, address: walletAddress };
    const payloadRes = await fetch(`${apiUrl}${AUTH_PAYLOAD_URL}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(authPayload),
    });
    const signingPayload = await payloadRes.json();

    // Step 2: Sign the SIWE with your wallet
    const wallet = new ethers.Wallet(walletKey);
    const signature = await wallet.signMessage(signingPayload.message);

    // Step 3: Connect with signature (chain_id and address required!)
    const connectRequest = {
        chain_id: chainId,
        address: walletAddress,
        message: signingPayload.message,
        nonce: signingPayload.nonce,
        mac: signingPayload.mac,
        ref_code,
        signature,
        issued_at: signingPayload.issued_at,
    };

    const connectRes = await fetch(`${apiUrl}${CONNECT_PAYLOAD_URL}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(connectRequest),
    });
    const connectResponse = await connectRes.json();
    const nonce = connectResponse.nonce;
    const authTokenCookie = connectRes.headers.get('set-cookie')?.match(/auth-token=([^;]+)/)?.[1];
    return { nonce, authTokenCookie };
}


async function makeAuthedRequest(apiUrl, nonce, authTokenCookie) {
    const url = `${apiUrl}${TEST_API}`;
    const res = await fetch(url, {
        headers: {
            'X-Auth-Nonce': nonce,
            'Cookie': `auth-token=${authTokenCookie}`,
        },
    });
    const data = await res.json();
    console.log(`Authed Request: ${url}`);
    console.log('Authed Response:', data);
}


async function main() {
    const { nonce, authTokenCookie } = await perplAuth(API_URL, PERPL_CHAIN_ID, WALLET_ADDRESS, WALLET_KEY);
    await makeAuthedRequest(API_URL, nonce, authTokenCookie);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
