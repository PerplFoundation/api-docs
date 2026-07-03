// Enrolling a fresh Ed25519 API key programmatically.
//
// MOST USERS DO NOT NEED THIS: just create a key in the web UI (connect wallet
// -> create key) at https://app.perpl.xyz/apikeys (testnet
// https://testnet.perpl.xyz/apikeys) and copy the token + private key.
//
// This script is for third-party integrations (trading terminals, bots) that
// enroll keys directly on behalf of a user's wallet. See ../../integrations.md
// for the full wallet-signed enrollment flow.
//
// An API key is an Ed25519 key pair. The server only stores the public key; the
// private key never leaves this machine. Enrollment is authorized once by the
// wallet's EIP-712 signature, after which every request is signed with the key's
// private key (see authed_rest_requests.js).
import { ethers } from 'ethers';
import * as ed from '@noble/ed25519';
import { fileURLToPath } from 'url';


const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';
const CHAIN_ID = Number(process.env.PERPL_CHAIN_ID) || 143;
// The Origin the key is enrolled from — must be whitelisted by Perpl. From a
// browser the Origin header is set automatically; from Node set it explicitly.
const ORIGIN = process.env.PERPL_ORIGIN || 'https://your-app.example';

const API_KEY_PAYLOAD_URL = '/v1/api-key/payload';
const API_KEY_ENROLL_URL = '/v1/api-key/enroll';

// API key scope bitmask: 1 = read, 2 = trade (implies read), 3 = both.
const SCOPE_MASK = 3;


const WALLET_KEY = process.env.OWNER_PRIVATE_KEY || '0xYourWalletPrivateKey';
const WALLET_ADDRESS = process.env.WALLET_ADDRESS || new ethers.Wallet(WALLET_KEY).address;


// Enroll a fresh Ed25519 API key, authorized by the wallet's EIP-712 signature.
// Returns { token, privateKey } where token is the opaque X-API-Key value and
// privateKey is the 32-byte Ed25519 secret used to sign every subsequent request.
async function enrollApiKey(apiUrl, chainId, origin, walletAddress, walletKey) {
    // Step 1: Generate the Ed25519 key pair (the private key never leaves the client).
    const privateKey = ed.utils.randomPrivateKey();
    const publicKey = await ed.getPublicKeyAsync(privateKey);
    const publicKeyHex = '0x' + Buffer.from(publicKey).toString('hex');

    // Step 2: Request the EIP-712 enrollment payload to sign.
    const payloadRes = await fetch(`${apiUrl}${API_KEY_PAYLOAD_URL}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Origin': origin },
        body: JSON.stringify({
            chain_id: chainId,
            address: walletAddress,
            public_key: publicKeyHex,
            scope_mask: SCOPE_MASK,
            label: 'example key',
        }),
    });
    const { typed_data, mac } = await payloadRes.json();

    // Step 3: Sign the typed data with the wallet (secp256k1 EIP-712 signature).
    // ethers wants the EIP-712 types WITHOUT the EIP712Domain entry.
    const wallet = new ethers.Wallet(walletKey);
    const { EIP712Domain, ...types } = typed_data.types;
    const signature = await wallet.signTypedData(typed_data.domain, types, typed_data.message);

    // Step 4: Ed25519 proof-of-possession over the same EIP-712 digest.
    const digest = ethers.TypedDataEncoder.hash(typed_data.domain, types, typed_data.message);
    const pop = await ed.signAsync(ethers.getBytes(digest), privateKey);
    const pop_signature = '0x' + Buffer.from(pop).toString('hex');

    // Step 5: Submit both signatures and receive the opaque X-API-Key token.
    const enrollRes = await fetch(`${apiUrl}${API_KEY_ENROLL_URL}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Origin': origin },
        body: JSON.stringify({
            chain_id: chainId,
            address: walletAddress,
            typed_data,
            mac,
            signature,
            pop_signature,
        }),
    });
    const enrollResponse = await enrollRes.json();
    const token = enrollResponse.api_key.api_key;
    return { token, privateKey };
}


async function main() {
    const { token, privateKey } = await enrollApiKey(API_URL, CHAIN_ID, ORIGIN, WALLET_ADDRESS, WALLET_KEY);
    const secretHex = Buffer.from(privateKey).toString('hex');
    console.log('Enrolled a new API key. Export these to use it with the other examples:');
    console.log('');
    console.log(`export PERPL_API_KEY=${token}`);
    console.log(`export PERPL_API_KEY_SECRET=0x${secretHex}`);
}


if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
