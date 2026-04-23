import { fileURLToPath } from 'url';

const API_URL = process.env.PERPL_API_URL ?? 'https://app.perpl.xyz/api';

const CONTEXT_URL = '/v1/pub/context';

interface Token {
    id: number;
    address: string;
    symbol: string;
    decimals: number;
}

interface SmartContractInstance {
    address: string;
    collateral_token_id: number;
    min_account_open_amount: string;
}

interface ExchangeContext {
    instances: SmartContractInstance[];
    tokens: Token[];
}

interface AccountCreationInfo {
    account_open_min_deposit_display: string;
    smart_contract_address: string;
    collateral_token_address: string;
    min_account_open_amount: number;
    collateral_token_symbol: string;
}

async function getAccountCreationInfo(apiUrl: string): Promise<AccountCreationInfo> {
    const response = await fetch(`${apiUrl}${CONTEXT_URL}`);
    const exchangeContext: ExchangeContext = await response.json();

    const smartContractInstance = exchangeContext.instances[0];
    const smartContractAddress = smartContractInstance.address;

    const collateralTokenId = smartContractInstance.collateral_token_id;
    const tokens = exchangeContext.tokens;
    const collateralToken = tokens.find((x) => x.id === collateralTokenId)!;
    const collateralTokenAddress = collateralToken.address;
    const collateralTokenSymbol = collateralToken.symbol;
    const collateralTokenDecimals = collateralToken.decimals;

    const minAccountOpenAmount = parseInt(smartContractInstance.min_account_open_amount);
    const accountOpenMinDepositFloat = minAccountOpenAmount / Math.pow(10, collateralTokenDecimals);
    const accountOpenMinDepositDisplay = `${accountOpenMinDepositFloat} ${collateralTokenSymbol}`;

    return {
        account_open_min_deposit_display: accountOpenMinDepositDisplay,
        smart_contract_address: smartContractAddress,
        collateral_token_address: collateralTokenAddress,
        min_account_open_amount: minAccountOpenAmount,
        collateral_token_symbol: collateralTokenSymbol,
    };
}

async function main(): Promise<void> {
    const accountCreationInfo = await getAccountCreationInfo(API_URL);
    console.log('Account Creation Info:');
    console.log(accountCreationInfo);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
    await main();
}
