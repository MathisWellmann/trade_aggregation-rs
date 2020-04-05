use crate::common::{Trade, Candle};

pub const M1: i64 = 60;  // 1 minute candle constant
pub const M5: i64 = 300;
pub const M15: i64 = 900;
pub const M30: i64 = 1800;
pub const H1: i64 = 3600;  // 1 hour candle constant
pub const H2: i64 = 7200;
pub const H4: i64 = 14400;
pub const H8: i64 = 28800;
pub const H12: i64 = 43200;
pub const D1: i64 = 86400;  // 1 day candle constant

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

    for i in 0..trades.len() {
        if init {
            init = false;
            init_timestamp = trades[i].timestamp;
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
            buy_volume += trades[i].size.abs()
        }
        wp += trades[i].price * trades[i].size.abs();

        if trades[i].timestamp - init_timestamp > threshold * 1000 {
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

            init_timestamp = trades[i].timestamp;
            open = trades[i].price;
            high = trades[i].price;
            low = trades[i].price;
            volume = 0.0;
            num_buys = 0;
            num_trades = 0;
        }
    }
    return out
}
