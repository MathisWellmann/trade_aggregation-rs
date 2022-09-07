//! This example shows how to aggregate all trades into time based
//! 1 minute candles all at once. The candle will contain the open, high, low and close price
//!
//! When deriving the 'Candle' macro, make sure the following things are in scope:
//! - Trade
//! - ModularCandle
//! - CandleComponent

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
    let mut aggregator = GenericAggregator::<MyCandle, TimeRule, Trade>::new(time_rule);

    let candles = aggregate_all_trades(&trades, &mut aggregator);
    println!("got {} candles", candles.len());
}
