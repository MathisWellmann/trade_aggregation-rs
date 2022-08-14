use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trade_aggregation::{
    aggregate_all_trades, load_trades_from_csv, By, ModularVolumeAggregator, Trade,
};

fn ohlc_modules_aggregation(trades: &[Trade]) {
    todo!()
    /*
    let mut ag = ModularVolumeAggregator::new(1000.0, By::Base);
    // Add the weighted price feature
    ag.add_feature(FeatureModules::WeightedPrice);

    let _candles = aggregate_all_trades(trades, &mut ag);
    */
}

fn criterion_benchmark(c: &mut Criterion) {
    let trades =
        load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("Could not open trade data file!");
    c.bench_function("ohlc_modules_aggregation", |b| {
        b.iter(|| ohlc_modules_aggregation(black_box(&trades)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
