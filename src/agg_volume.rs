use crate::common::{Trade, Candle};

// agg_volume aggregates trades by volume
pub fn agg_volume(trades: &Vec<Trade>, threshold: f64) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();

    let mut open = trades[0].price;
    let mut high = trades[0].price;
    let mut low = trades[0].price;
    let mut volume = trades[0].size.abs();
    let mut buy_volume: f64 = 0.0;
    let mut num_buys: i8 = 0;
    let mut num_trades: i8 = 0;
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
        volume += trades[i].size.abs();
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
