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
