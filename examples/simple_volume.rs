extern crate trade_aggregation;
use trade_aggregation::{aggregate_all_trades, load_trades_from_csv, By, VolumeAggregator};

fn main() {
    // load trades from file
    let trades =
        load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("Trade data file not found");
    // create the desired volume aggregator with parameters
    let mut aggregator = VolumeAggregator::new(1000.0, By::Base);

    let candles = aggregate_all_trades(&trades, &mut aggregator);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
