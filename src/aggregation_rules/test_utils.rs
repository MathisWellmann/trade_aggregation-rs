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
