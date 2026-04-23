import os
import requests
import pprint


API_URL = os.environ.get("PERPL_API_URL", "https://app.perpl.xyz/api")
MONAD_RPC = os.environ.get("PERPL_RPC_URL", "https://rpc.monad.xyz")


CONTEXT_URL = "/v1/pub/context"


def get_account_creation_info(api_url):
    url = f"{api_url}{CONTEXT_URL}"

    response = requests.get(url)
    exchange_context = response.json()

    smart_contract_instance = exchange_context["instances"][0]
    smart_contract_address = smart_contract_instance["address"]

    collateral_token_id = smart_contract_instance["collateral_token_id"]
    tokens = exchange_context["tokens"]
    collateral_token = [x for x in tokens if x["id"] == collateral_token_id][0]
    collateral_token_address = collateral_token["address"]
    collateral_token_symbol = collateral_token["symbol"]
    collateral_token_decimals = collateral_token["decimals"]

    min_account_open_amount = int(smart_contract_instance["min_account_open_amount"])
    account_open_min_deposit_float = min_account_open_amount / pow(10, collateral_token_decimals)
    account_open_min_deposit_display = f"{account_open_min_deposit_float} {collateral_token_symbol}"

    return {
        "account_open_min_deposit_display": account_open_min_deposit_display,
        "smart_contract_address": smart_contract_address,
        "min_account_open_amount": min_account_open_amount,
        "collateral_token_symbol": collateral_token_symbol,
        "collateral_token_address": collateral_token_address,
    }


def main():
    account_creation_info = get_account_creation_info(API_URL)
    print("Account Creation Info:")
    pprint.pprint(account_creation_info)


if __name__ == "__main__":
    main()
