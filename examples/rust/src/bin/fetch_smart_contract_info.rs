use anyhow::{anyhow, Result};
use serde::Deserialize;

const DEFAULT_API_URL: &str = "https://app.perpl.xyz/api";
const CONTEXT_URL: &str = "/v1/pub/context";


#[derive(Deserialize)]
struct SmartContractInstance {
    address: String,
    collateral_token_id: i64,
    min_account_open_amount: String
}

#[derive(Deserialize)]
struct CollateralToken {
    id: i64,
    address: String,
    symbol: String,
    decimals: u32,
}

#[derive(Deserialize)]
struct ExchangeContextResponse {
    instances: Vec<SmartContractInstance>,
    tokens: Vec<CollateralToken>
}

#[derive(Debug)]
struct AccountCreationInfo {
    account_open_min_deposit_display: String,
    smart_contract_address: String,
    min_account_open_amount: u64,
    collateral_token_address: String,
    collateral_token_symbol: String,
}

async fn get_account_creation_info(api_url: &str) -> Result<AccountCreationInfo> {
    let client = reqwest::Client::new();
    let url = format!("{}{}", api_url, CONTEXT_URL);

    let response = client.get(&url).send().await?;
    let exchange_context: ExchangeContextResponse = serde_json::from_str(response.text().await?.as_str())?;

    let smart_contract_instance = exchange_context.instances.first().ok_or_else(|| anyhow!("no smart contract instance"))?;
    let smart_contract_address = &smart_contract_instance.address;
    let collateral_token_id = &smart_contract_instance.collateral_token_id;

    let collateral_token = exchange_context.tokens.iter().find(|t|t.id == *collateral_token_id).ok_or_else(|| anyhow!("no collateral token"))?;

    let min_account_open_amount: u64 = smart_contract_instance.min_account_open_amount.as_str().parse()?;
    let account_open_min_deposit_float =
        min_account_open_amount as f64 / 10u64.pow(collateral_token.decimals) as f64;
    let account_open_min_deposit_display = format!("{} {}", account_open_min_deposit_float, collateral_token.symbol);

    Ok(AccountCreationInfo {
        account_open_min_deposit_display,
        smart_contract_address: smart_contract_address.to_string(),
        min_account_open_amount,
        collateral_token_address: collateral_token.address.to_string(),
        collateral_token_symbol: collateral_token.symbol.to_string(),
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_url = std::env::var("PERPL_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
    let account_creation_info = get_account_creation_info(&api_url).await?;
    println!("Account Creation Info:");
    println!("{:?}", account_creation_info);
    Ok(())
}
