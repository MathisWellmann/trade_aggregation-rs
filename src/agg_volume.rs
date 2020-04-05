use crate::common::{Trade, Candle};

const ASSET: usize = 0;
const BASE: usize = 0;

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

    for i in 1..trades.len() {
        if init {
            init = false;

            open = trades[i].price;
            high = trades[i].price;
            low = trades[i].price;
            volume = 0.0;
            buy_volume = 0.0;
            num_buys = 0;
            num_trades = 0;
            wp = 0.0;
        }

        if trades[i].price > high {
            high = trades[i].price
        } else if trades[i].price < low {
            low = trades[i].price
        }
        if by == ASSET {
            volume += trades[i].size.abs() / trades[i].price;
            if trades[i].size > 0.0 {
                buy_volume += trades[i].size.abs() / trades[i].price;
            }
            wp += trades[i].size.abs();
        } else if by == BASE {
            volume += trades[i].size.abs();
            if trades[i].size > 0.0 {
                buy_volume += trades[i].size.abs();
            }
            wp += trades[i].size.abs() * trades[i].price;
        }

        num_trades += 1;
        if trades[i].size > 0.0 {
            num_buys += 1;
        }

        if volume > threshold {
            // create new candle
            let c = Candle{
                timestamp: trades[i].timestamp,
                open,
                high,
                low,
                close: trades[i].price,
                volume,
                volume_direction_ratio: buy_volume / volume,
                trade_direction_ratio: num_buys as f64 / num_trades as f64,
                num_trades,
                weighted_price: wp / volume,
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
