use trade_aggregation::{load_trades_from_csv, By, FeatureModules, ModularVolumeAggregator};

fn main() {
    // load trades from file
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");

    let mut agg_volume = ModularVolumeAggregator::new(1000.0, By::Base);
    // Add the weighted price feature
    agg_volume.add_feature(FeatureModules::WeightedPrice);

    for t in &trades {
        // update using latest trade
        match agg_volume.update(t) {
            Some(candle) => {
                // do something with the latest candle
                println!("candle: {:?}", candle);
            }
            None => {}
        }
    }
}
