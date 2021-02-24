extern crate trade_aggregation;
use trade_aggregation::{load_trades_from_csv, Aggregator, VolumeAggregator, By};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    // create new streaming aggregator based on volume
    let mut agg_volume = VolumeAggregator::new(1000.0, By::Base);

    for t in &trades {
        // update using the latest trade
        match agg_volume.update(t) {
            Some(candle) => {
                // do something with the latest candle
                println!("candle: {:?}", candle);
            },
            None => {}
        }
    }
}
