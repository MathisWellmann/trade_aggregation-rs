extern crate trade_aggregation;
use trade_aggregation::{common, agg_time_streaming};
use trade_aggregation::agg_time_streaming::AggTimeStreaming;

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    // create new streaming aggregator based on time
    // Candle period can be accessed with constants in common
    // H1 are hourly candles
    let mut agg_time = AggTimeStreaming::new(common::H1);

    for i in 0..trades.len() {
        // update using the latest trade
        let new_candle = agg_time.update(&trades[i]);
        // if new candle has been created
        if new_candle {
            // access latest candle
            let candle = agg_time.last();
            println!("candle: {:?}", candle);
        }
    }
}
