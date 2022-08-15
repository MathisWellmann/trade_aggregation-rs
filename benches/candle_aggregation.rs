use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trade_aggregation::{
    aggregate_all_trades, candle_components::Open, load_trades_from_csv, By, CandleComponent,
    GenericAggregator, ModularCandle, TimeRule, Trade, M1,
};
use trade_aggregation_derive::Candle;

#[derive(Debug, Clone, Default, Candle)]
struct CandleOpen {
    open: Open,
}

/*
#[derive(Debug, Clone, Default, Candle)]
struct CandleOHLC {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}
*/

fn time_aggregation_open(trades: &[Trade]) {
    let time_rule = TimeRule::new(M1);
    let mut aggregator = GenericAggregator::<CandleOpen, TimeRule>::new(time_rule);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

/*
fn volume_aggregation(trades: &[Trade]) {
    let volume_rule = VolumeRule::new(10.0, By::Base);
    let mut aggregator = GenericAggregator::<CandleOpen, VolumeRule>::new(volume_rule);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}
*/

fn criterion_benchmark(c: &mut Criterion) {
    let trades =
        load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("Could not open trade data file!");
    c.bench_function("time_aggregation", |b| {
        b.iter(|| time_aggregation_open(black_box(&trades)))
    });
    /*
    c.bench_function("volume_aggregation", |b| {
        b.iter(|| volume_aggregation(black_box(&trades)))
    });
    */
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
