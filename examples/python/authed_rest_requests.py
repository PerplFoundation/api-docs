# Authenticating with the API and making a REST request
import os
import requests

from eth_account import Account
from eth_account.messages import encode_defunct


API_URL = os.environ.get("PERPL_API_URL", "https://app.perpl.xyz/api")
AUTH_PAYLOAD_URL = "/v1/auth/payload"
CONNECT_PAYLOAD_URL = "/v1/auth/connect"
PERPL_CHAIN_ID = int(os.environ.get("PERPL_CHAIN_ID", 143))
TEST_API = "/v1/profile/contact-info"


WALLET_ADDRESS = '0xYourWalletAddress'
WALLET_KEY = '0xYourWalletPrivateKey'


def perpl_auth(api_url, chain_id, wallet_address, wallet_key, ref_code=""):
    # Step 1: Get signing payload
    auth_payload = {
        "chain_id": chain_id,
        "address": wallet_address,
    }
    url = api_url + AUTH_PAYLOAD_URL
    response = requests.post(url, json=auth_payload)
    signing_payload = response.json()

    # Step 2: Sign the SIWE with your wallet
    account = Account.from_key(wallet_key)
    message = signing_payload["message"]
    signed_message = account.sign_message(encode_defunct(text=message))

    # Step 3: Connect with signature (chain_id and address required!)
    connect_url = api_url + CONNECT_PAYLOAD_URL
    connect_request = {
        "chain_id": chain_id,
        "address": wallet_address,
        "message": message,
        "nonce": signing_payload["nonce"],
        "mac": signing_payload["mac"],
        "ref_code": ref_code,
        "signature": "0x" + signed_message.signature.hex(),
        "issued_at": signing_payload["issued_at"],
    }

    response = requests.post(connect_url, json=connect_request)
    connect_response = response.json()
    nonce = connect_response["nonce"]
    auth_token_cookie = response.cookies["auth-token"]
    return nonce, auth_token_cookie


def make_authed_request(api_url, nonce, auth_token_cookie):
    session = requests.Session()
    session.headers.update({"X-Auth-Nonce": nonce})
    session.cookies.update({"auth-token": auth_token_cookie})

    url = api_url + TEST_API
    response = session.get(url)
    data = response.json()
    print(f"Authed Request: {url}")
    print(f"Authed Response: {data}")


def main():
    nonce, auth_token_cookie = perpl_auth(API_URL, PERPL_CHAIN_ID, WALLET_ADDRESS, WALLET_KEY)
    make_authed_request(API_URL, nonce, auth_token_cookie)


if __name__ == "__main__":
    main()
