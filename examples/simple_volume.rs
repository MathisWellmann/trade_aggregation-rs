extern crate trade_aggregation;
use trade_aggregation::{common, agg_volume, By};

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    let threshold = 1000.0;  // ~volume in each candle
    let by = By::Base;  // take USD as volume measure
    let candles = agg_volume::agg_volume(&trades, threshold, by);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
