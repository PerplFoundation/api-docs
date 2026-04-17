// Making REST requests to public APIs without authentication
const API_URL = process.env.PERPL_API_URL || 'https://app.perpl.xyz/api';


// Fetch exchange context
const contextRes = await fetch(`${API_URL}/v1/pub/context`);
const exchangeContext = await contextRes.json();

console.log('Exchange Context:', exchangeContext);
console.log('Exchange Context Keys:', Object.keys(exchangeContext));
console.log();

// Fetch candles
const markets = exchangeContext.markets;
const btcMarket = markets.find(m => m.name === 'BTC');
const btcMarketId = btcMarket.id;

// Scale for pricing, API gives integers so need to add decimal places
const priceDecimals = btcMarket.config.price_decimals;
const priceScale = Math.pow(10, priceDecimals);

const resolution = 3600; // Valid resolutions: 60, 300, 900, 1800, 3600, 7200, 14400, 28800, 43200, 86400

const nCandles = 10;
const toMs = Date.now();
const fromMs = toMs - resolution * nCandles * 1000;

const candlesRes = await fetch(`${API_URL}/v1/market-data/${btcMarketId}/candles/${resolution}/${fromMs}-${toMs}`);
const candles = await candlesRes.json();

console.log('Candles:');
for (const candle of candles.d) {
    const ts = candle.t;
    const openPrice = candle.o / priceScale;
    const closePrice = candle.c / priceScale;
    const highPrice = candle.h / priceScale;
    const lowPrice = candle.l / priceScale;
    const volume = candle.v;
    const nTrades = candle.n;
    console.log(`ts: ${ts} open_price: ${openPrice} close_price: ${closePrice} high_price: ${highPrice} low_price: ${lowPrice} volume: ${volume} n_trades: ${nTrades}`);
}
