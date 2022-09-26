use criterion::{black_box, criterion_group, criterion_main, Criterion};
use trade_aggregation::{
    aggregate_all_trades, candle_components::*, load_trades_from_csv, CandleComponent,
    GenericAggregator, ModularCandle, TimeRule, TimestampResolution, Trade, M1,
};
use trade_aggregation_derive::Candle;

#[derive(Debug, Clone, Default, Candle)]
struct CandleOpen {
    open: Open,
}

#[derive(Debug, Clone, Default, Candle)]
struct CandleOHLC {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}

#[derive(Debug, Clone, Default, Candle)]
struct CandleAll {
    open: Open,
    high: High,
    low: Low,
    close: Close,
    volume: Volume,
    num_trades: NumTrades,
    directional_trade_ratio: DirectionalTradeRatio,
    directional_volume_ratio: DirectionalVolumeRatio,
    std_dev_prices: StdDevPrices,
    std_dev_sizes: StdDevSizes,
    weighted_price: WeightedPrice,
    average_price: AveragePrice,
    time_velocity: TimeVelocity,
}

fn time_aggregation_open(trades: &[Trade]) {
    let time_rule = TimeRule::new(M1, TimestampResolution::Millisecond);
    let mut aggregator = GenericAggregator::<CandleOpen, TimeRule, Trade>::new(time_rule);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

fn time_aggregation_ohlc(trades: &[Trade]) {
    let time_rule = TimeRule::new(M1, TimestampResolution::Millisecond);
    let mut aggregator = GenericAggregator::<CandleOHLC, TimeRule, Trade>::new(time_rule);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

fn time_aggregation_all(trades: &[Trade]) {
    let time_rule = TimeRule::new(M1, TimestampResolution::Millisecond);
    let mut aggregator = GenericAggregator::<CandleAll, TimeRule, Trade>::new(time_rule);
    let _candles = aggregate_all_trades(trades, &mut aggregator);
}

fn criterion_benchmark(c: &mut Criterion) {
    let trades =
        load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("Could not open trade data file!");
    c.bench_function("time_aggregation_open", |b| {
        b.iter(|| time_aggregation_open(black_box(&trades)))
    });
    c.bench_function("time_aggregation_ohlc", |b| {
        b.iter(|| time_aggregation_ohlc(black_box(&trades)))
    });
    c.bench_function("time_aggregation_all", |b| {
        b.iter(|| time_aggregation_all(black_box(&trades)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
