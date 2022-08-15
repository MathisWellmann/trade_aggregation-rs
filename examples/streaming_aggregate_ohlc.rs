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
    let time_rule = TimeRule::new(M1);
    let mut aggregator = GenericAggregator::<MyCandle, TimeRule>::new(time_rule);

    for t in &trades {
        if let Some(candle) = aggregator.update(t) {
            println!(
                "candle created with open: {}, high: {}, low: {}, close: {}",
                candle.open(),
                candle.high(),
                candle.low(),
                candle.close()
            );
        }
    }
}
