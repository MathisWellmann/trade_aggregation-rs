//! This example shows how to perform tick-by-tick streaming trade aggregation
//!

use trade_aggregation::{
    candle_components::{Close, High, Low, Open},
    *,
};

#[derive(Debug, Default, Clone, Candle)]
struct MyCandle {
    open: Open,
    high: High,
    low: Low,
    close: Close,
}

fn main() {
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
        .expect("Could not load trades from file!");

    // specify the aggregation rule to be time based
    let time_rule = TimeRule::new(M1, TimestampResolution::Millisecond);
    let mut aggregator = GenericAggregator::<MyCandle, TimeRule, Trade>::new(time_rule, false);

    // Receives the candles that each trade closes; reused to avoid allocations.
    let mut closed = Vec::new();
    for t in &trades {
        aggregator.update(t, &mut closed);
        for bar in closed.drain(..) {
            let candle = bar.candle();
            println!(
                "candle {} closed at {} with open: {}, high: {}, low: {}, close: {}",
                bar.seq(),
                bar.close_ts(),
                candle.open(),
                candle.high(),
                candle.low(),
                candle.close()
            );
        }
    }
}
