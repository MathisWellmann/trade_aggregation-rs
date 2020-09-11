use trade_aggregation::common;
use trade_aggregation::agg_volume_streaming_modular::AggVolumeStreamingModular;
use trade_aggregation::modules::FeatureModules;
use trade_aggregation::By;

fn main() {
    // load trades from file
    let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    let threshold = 10.0;
    let by = By::Base;

    let mut agg_volume = AggVolumeStreamingModular::new(threshold, by);
    // Just calculate weighted price
    agg_volume.add_feature(FeatureModules::WeightedPrice);

    for t in &trades {
        // update using latest trade
        let new_candle = agg_volume.update(t);
        // if new candles has been created
        if new_candle {
            // access latest candle
            let candle = agg_volume.last();
            println!("candle: {:?}", candle);
        }
    }
}