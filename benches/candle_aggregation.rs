use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trade_aggregation::{
    aggregate_all_trades, load_trades_from_csv, By, TimeAggregator, Trade, VolumeAggregator, M1,
};

fn time_aggregation(trades: &[Trade]) {
    let mut aggregator = TimeAggregator::new(M1);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

fn volume_aggregation(trades: &[Trade]) {
    let mut aggregator = VolumeAggregator::new(10.0, By::Base);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

fn criterion_benchmark(c: &mut Criterion) {
    let trades =
        load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("Could not open trade data file!");
    c.bench_function("time_aggregation", |b| {
        b.iter(|| time_aggregation(black_box(&trades)))
    });
    c.bench_function("volume_aggregation", |b| {
        b.iter(|| volume_aggregation(black_box(&trades)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
