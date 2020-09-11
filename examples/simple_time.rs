extern crate trade_aggregation;
use trade_aggregation::{common, agg_time};

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    let candle_period = common::H1;
    let candles = agg_time::agg_time(&trades, candle_period);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
