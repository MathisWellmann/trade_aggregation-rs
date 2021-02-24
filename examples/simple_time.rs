extern crate trade_aggregation;
use trade_aggregation::{aggregate_all_trades, load_trades_from_csv, H1, TimeAggregator};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
    let mut aggregator = TimeAggregator::new(H1);
    let candles = aggregate_all_trades(&trades, &mut aggregator);

    for c in &candles {
        println!("candle: {:?}", c);
    }
}
