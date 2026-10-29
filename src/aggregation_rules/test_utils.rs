//! Helpers for the tests of the aggregation rules.

use crate::{plot::OhlcCandle, AggregationRule, Trade};

/// A trade with a millisecond timestamp.
pub(crate) fn trade(timestamp: i64, price: f64, size: f64) -> Trade {
    Trade {
        timestamp,
        price,
        size,
    }
}

/// Trades one millisecond apart at the given prices, each buying one unit.
pub(crate) fn trades_at_prices(prices: &[f64]) -> Vec<Trade> {
    prices
        .iter()
        .zip(0..)
        .map(|(price, ts)| trade(ts, *price, 1.0))
        .collect()
}

/// The indices of the trades on which `rule` triggers.
pub(crate) fn trigger_indices<R: AggregationRule<OhlcCandle, Trade>>(
    rule: &mut R,
    trades: &[Trade],
) -> Vec<usize> {
    let candle = OhlcCandle::default();
    trades
        .iter()
        .enumerate()
        .filter_map(|(i, t)| rule.should_trigger(t, &candle).then_some(i))
        .collect()
}

/// `n` trades of size 1, one millisecond apart at a price of 100,
/// each a buy with probability `p_buy`, drawn with a seeded xorshift generator.
pub(crate) fn random_flow(n: i64, p_buy: f64, seed: u64) -> Vec<Trade> {
    assert!(seed != 0, "xorshift needs a non-zero seed");
    let mut state = seed;
    (0..n)
        .map(|ts| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let uniform = (state >> 11) as f64 / (1_u64 << 53) as f64;
            trade(ts, 100.0, if uniform < p_buy { 1.0 } else { -1.0 })
        })
        .collect()
}
