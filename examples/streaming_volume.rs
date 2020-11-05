extern crate trade_aggregation;
use trade_aggregation::{common, By};
use trade_aggregation::agg_volume_streaming::AggVolumeStreaming;

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    // create new streaming aggregator based on volume
    let threshold = 1000.0;  // threshold of volume in this case denoted in BASE currency which is USD
    let by = By::Quote;  // take USD as volume measure
    // let by = By::Base;  // take BTC as volume measure
    let mut agg_volume = AggVolumeStreaming::new(threshold, by);

    for t in &trades {
        // update using the latest trade
        let new_candle = agg_volume.update(t);
        // if new candle has been created
        if new_candle {
            // access latest candle
            let candle = agg_volume.last();
            println!("candle: {:?}", candle);
        }
    }
}
