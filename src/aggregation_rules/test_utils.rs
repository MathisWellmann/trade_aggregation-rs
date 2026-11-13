//! Helpers for the tests of the aggregation rules.

use crate::{
    AggregationRule, MarketEvent, Quote, SizeUnit, TakerTrade, TimestampResolution, Timestamped,
    Trade,
};

/// A stream of trades and quotes, for the rules that follow the mid price or the microprice.
pub(crate) type TradeOrQuote = MarketEvent<Trade, Quote>;

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

/// Prices that move by the given log returns from 100.
pub(crate) fn prices_from_returns(returns: &[f64]) -> Vec<f64> {
    let mut price = 100.0;
    let mut prices = Vec::with_capacity(1 + returns.len());
    prices.push(price);
    for r in returns {
        price *= r.exp();
        prices.push(price);
    }
    prices
}

/// The indices of the events (e.g. trades) on which `rule` triggers.
pub(crate) fn trigger_indices<E, R: AggregationRule<E>>(rule: &mut R, events: &[E]) -> Vec<usize> {
    events
        .iter()
        .enumerate()
        .filter_map(|(i, t)| rule.should_trigger(t).then_some(i))
        .collect()
}

/// A trade of one unit at `price` in a stream of trades and quotes.
pub(crate) fn trade_event(price: f64) -> TradeOrQuote {
    MarketEvent::Trade(trade(0, price, 1.0))
}

/// A quote in a stream of trades and quotes.
pub(crate) fn quote_event(bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> TradeOrQuote {
    MarketEvent::Quote(Quote {
        timestamp: 0,
        bid_price: bid,
        bid_size,
        ask_price: ask,
        ask_size,
    })
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

/// A [`Trade`] whose timestamp is in microseconds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MicroTrade(pub(crate) Trade);

impl Timestamped for MicroTrade {
    const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Microsecond;

    fn timestamp(&self) -> i64 {
        self.0.timestamp
    }
}

impl TakerTrade for MicroTrade {
    const SIZE_UNIT: SizeUnit = SizeUnit::Quote;

    fn price(&self) -> f64 {
        self.0.price
    }

    fn size(&self) -> f64 {
        self.0.size
    }
}

/// A [`Trade`] whose timestamp is in nanoseconds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NanoTrade(pub(crate) Trade);

impl Timestamped for NanoTrade {
    const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Nanosecond;

    fn timestamp(&self) -> i64 {
        self.0.timestamp
    }
}

impl TakerTrade for NanoTrade {
    const SIZE_UNIT: SizeUnit = SizeUnit::Quote;

    fn price(&self) -> f64 {
        self.0.price
    }

    fn size(&self) -> f64 {
        self.0.size
    }
}

/// The number of time candles that close with the events at `timestamps`,
/// for periods of `period` units counted from `origin`, without empty candles:
/// one less than the number of periods that hold an event.
pub(crate) fn closed_periods(timestamps: &[i64], origin: i64, period: i64) -> usize {
    let mut periods: Vec<i64> = timestamps
        .iter()
        .map(|ts| (ts - origin).div_euclid(period))
        .collect();
    periods.dedup();
    assert!(periods.is_sorted(), "timestamps in time order");
    periods.len().saturating_sub(1)
}
