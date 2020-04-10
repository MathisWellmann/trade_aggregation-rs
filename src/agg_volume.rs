use crate::common::{Trade, Candle, BASE, ASSET};
use crate::welford_online;

// agg_volume aggregates trades by volume
pub fn agg_volume(trades: &Vec<Trade>, threshold: f64, by: usize) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();

    let mut open = trades[0].price;
    let mut high = trades[0].price;
    let mut low = trades[0].price;
    let mut volume = trades[0].size.abs();
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
        if by == ASSET {
            volume += t.size.abs() / t.price;
            if t.size > 0.0 {
                buy_volume += t.size.abs() / t.price;
            }
            wp += t.size.abs();
        } else if by == BASE {
            volume += t.size.abs();
            if t.size > 0.0 {
                buy_volume += t.size.abs();
            }
            wp += t.size.abs() * t.price;
        }

        num_trades += 1;
        if t.size > 0.0 {
            num_buys += 1;
        }
        welford_prices.add(t.price);
        welford_sizes.add(t.size);

        if volume > threshold {
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
    fn test_agg_volume_base() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let agg_volume = agg_volume(&trades, 1000.0, BASE);

        for i in 0..agg_volume.len() {
            common::test_candle(&agg_volume[i]);
        }
    }

    #[test]
    fn test_agg_volume_asset() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let agg_volume = agg_volume(&trades, 1000.0, ASSET);

        for i in 0..agg_volume.len() {
            common::test_candle(&agg_volume[i]);
        }
    }
}
