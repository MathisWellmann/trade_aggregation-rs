use crate::common::{Trade, Candle};
use crate::welford_online;

// agg_time aggregates trades by timestamp and returns a vector of candles
// threshold in nano-seconds
pub fn agg_time(trades: &Vec<Trade>, threshold: i64) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();

    let mut init_timestamp: i64 = trades[0].timestamp;
    let mut open: f64 = trades[0].price;
    let mut high: f64 = trades[0].price;
    let mut low: f64 = trades[0].price;
    let mut volume: f64 = 0.0;
    let mut buy_volume: f64 = 0.0;
    let mut num_buys: i32 = 0;
    let mut num_trades: i32 = 0;
    let mut wp: f64 = 0.0;
    let mut init: bool = true;
    let mut welford_prices = welford_online::new();
    let mut welford_sizes = welford_online::new();

    for t in trades.iter() {
        if init {
            init = false;
            init_timestamp = t.timestamp;
            open = t.price;
            high = t.price;
            low = t.price;
            volume = 0.0;
            buy_volume = 0.0;
            num_buys = 0;
            num_trades = 0;
            wp = 0.0;
            welford_prices.reset();
            welford_sizes.reset();
        }
        if t.price > high {
            high = t.price
        } else if t.price < low {
            low = t.price
        }
        volume += t.size.abs();
        num_trades += 1;
        if t.size > 0.0 {
            num_buys += 1;
            buy_volume += t.size.abs()
        }
        wp += t.price * t.size.abs();

        welford_prices.add(t.price);
        welford_sizes.add(t.size);

        // convert threshold from seconds to milliseconds
        if t.timestamp - init_timestamp > threshold * 1000 {
            // create new candle
            let c = Candle{
                timestamp: t.timestamp,
                open,
                high,
                low,
                close: t.price,
                volume,
                volume_direction_ratio: buy_volume / volume,
                trade_direction_ratio: num_buys as f64 / num_trades as f64,
                num_trades,
                weighted_price: wp / volume,
                std_dev_prices: welford_prices.std_dev(),
                std_dev_sizes: welford_sizes.std_dev(),
            };
            out.push(c);

            init = true;
        }
    }
    return out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_agg_time() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let candles = agg_time(&trades, common::H1);

        for i in 0..candles.len() {
            common::test_candle(&candles[i]);
        }
    }
}
