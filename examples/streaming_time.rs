extern crate trade_aggregation;
use trade_aggregation::TimeAggregator;
use trade_aggregation::{load_trades_from_csv, Aggregator, H1};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    // create new streaming aggregator based on time
    let mut agg_time = TimeAggregator::new(H1);

    for t in &trades {
        // update using the latest trade
        match agg_time.update(t) {
            Some(candle) => {
                // use the latest candle in some way
                println!("candle: {:?}", candle)
            }
            None => {}
        };
    }
}
