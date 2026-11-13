//! The shape of the path of a [`PriceSource`] within a candle: how straight it went
//! ([`EfficiencyRatio`]) and whether it reverts ([`ReturnAutocovariance`]).
//!
//! Open, high, low and close lose the order of the moves within a candle:
//! a candle that rose then fell has the same OHLC as one that fell then rose.
//! These components keep that order as scalars that a model can use.
//! Like [`RealizedVariance`](super::RealizedVariance), the returns of a candle start at the last
//! price of the previous candle when there is one, so the move into the first price counts.

use crate::{
    price_source::LogReturns, CandleComponent, CandleComponentUpdate, PriceSource, TradePrice,
};

/// The efficiency ratio `|sum r| / sum |r|` of the log returns of the candle, in `[0, 1]`:
/// the net move over the distance travelled. 1 for a straight trend, near 0 for a chop that
/// ends where it started. `None` when the price did not move (`sum |r| = 0`).
/// Scale-free. Not additive itself, but its parts are: the denominator is the
/// [`TotalVariation`](super::TotalVariation) and the numerator the absolute log return from the
/// previous close, so a window of candles gives `|ln(close / previous close)| / sum TV`.
///
/// Source: the efficiency ratio of Kaufman (1995), "Smarter Trading", which drives his adaptive
/// moving average.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EfficiencyRatio<P = TradePrice> {
    net: f64,
    total: f64,
    returns: LogReturns<P>,
}

impl<P> CandleComponent for EfficiencyRatio<P> {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if self.total == 0.0 {
            return None;
        }
        let ratio = self.net.abs() / self.total;
        // Rounding is monotone, so |sum r| <= sum |r| also holds for the rounded sums.
        debug_assert!((0.0..=1.0).contains(&ratio), "{ratio}");
        Some(ratio)
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.net = 0.0;
        self.total = 0.0;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for EfficiencyRatio<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return;
        };
        self.net += r;
        self.total += r.abs();
        debug_assert!(self.total >= self.net.abs());
    }
}

/// The sum `sum r_t * r_(t-1)` of the products of consecutive log returns, the numerator of
/// their first order autocovariance. Negative when the price reverts, e.g. the trade price that
/// bounces between the bid and the ask; positive when moves continue.
/// The product of the first return of a candle with the last return of the previous candle
/// counts in the candle, like in [`BipowerVariation`](super::BipowerVariation), so the sum is
/// additive. [`roll_spread`] turns it into a spread estimate.
///
/// Source: Roll (1984), "A simple implicit measure of the effective bid-ask spread in an
/// efficient market".
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ReturnAutocovariance<P = TradePrice> {
    sum: f64,
    returns: LogReturns<P>,
    last_return: Option<f64>,
}

impl<P> CandleComponent for ReturnAutocovariance<P> {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.sum
    }

    /// Keeps the last price and return, for the first product of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for ReturnAutocovariance<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return;
        };
        if let Some(previous) = self.last_return.replace(r) {
            self.sum += r * previous;
        }
        debug_assert!(self.sum.is_finite());
    }
}

/// The relative bid-ask spread estimate of Roll (1984), `2 * sqrt(-cov)`, as a fraction of the
/// price, from the [`ReturnAutocovariance`] sum and the [`ReturnCount`](super::ReturnCount) `n`
/// of the same price source (usually the trade price), over a candle or a window of candles.
///
/// `cov = autocovariance / n` is the first order autocovariance of the log returns. It takes
/// the mean return as zero, which is negligible against the bounce at tick frequency and keeps
/// the sums additive. It divides by `n`, the number of returns: each return is paired with the
/// previous one, also across candles, except the first return of the series.
///
/// `None` with fewer than 2 returns, or when `cov >= 0`: the price moves did not revert,
/// so the model of a bounce around a random walk does not hold.
///
/// # Panics
/// If `autocovariance` is not finite.
#[must_use]
pub fn roll_spread(n: u64, autocovariance: f64) -> Option<f64> {
    assert!(autocovariance.is_finite(), "{autocovariance}");
    if n < 2 {
        return None;
    }
    let cov = autocovariance / n as f64;
    if cov >= 0.0 {
        return None;
    }
    let spread = 2.0 * (-cov).sqrt();
    debug_assert!(spread > 0.0);
    Some(spread)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, trades_at_prices},
        candle_components::{ReturnCount, TotalVariation},
        MarketEvent, Microprice, MidPrice, Quote, Trade,
    };

    fn update_all<E, C: CandleComponentUpdate<E>>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    /// The value of a fresh component over trades at `prices`.
    fn value_of<C>(prices: &[f64]) -> C::Value
    where
        C: CandleComponent + CandleComponentUpdate<Trade> + Default,
    {
        let mut c = C::default();
        update_all(&mut c, &trades_at_prices(prices));
        c.value()
    }

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-15, "{a} != {b}");
    }

    #[test]
    fn efficiency_ratio_of_trends_and_chops() {
        let ratio = |prices: &[f64]| value_of::<EfficiencyRatio>(prices);
        assert_close(ratio(&[100.0, 101.0, 103.0, 106.0]).expect("moves"), 1.0);
        assert!(
            ratio(&[100.0, 101.0, 100.0]).expect("moves") < 1e-12,
            "a round trip"
        );
        let (up, down) = ((102.0_f64 / 100.0).ln(), (101.0_f64 / 102.0).ln());
        assert_close(
            ratio(&[100.0, 102.0, 101.0]).expect("moves"),
            (up + down).abs() / (up - down),
        );
        assert_eq!(ratio(&[100.0, 100.0]), None, "no move");
        assert_eq!(ratio(&[100.0]), None, "no return");
        assert_eq!(ratio(&[]), None);
    }

    #[test]
    fn efficiency_ratio_starts_at_the_previous_close() {
        let mut ratio = EfficiencyRatio::<TradePrice>::default();
        update_all(&mut ratio, &trades_at_prices(&[100.0, 102.0]));
        ratio.reset();
        assert_eq!(ratio.value(), None, "an empty candle");
        update_all(&mut ratio, &trades_at_prices(&[101.0, 102.0]));
        assert!(ratio.value().expect("moves") < 1e-12, "102 -> 101 -> 102");
    }

    #[test]
    fn autocovariance_multiplies_consecutive_returns() {
        let prices = [100.0_f64, 101.0, 99.0, 99.0, 102.0, 100.0];
        let r: Vec<f64> = prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let expected: f64 = r.windows(2).map(|w| w[0] * w[1]).sum();
        let whole = value_of::<ReturnAutocovariance>(&prices);
        assert_close(whole, expected);
        assert!(whole < 0.0, "the path reverts");
        // The product across a boundary counts in the later candle, so the sum is additive.
        let trades = trades_at_prices(&prices);
        for at in 0..=trades.len() {
            let mut c = ReturnAutocovariance::<TradePrice>::default();
            update_all(&mut c, &trades[..at]);
            let first = c.value();
            c.reset();
            assert_eq!(c.value(), 0.0);
            update_all(&mut c, &trades[at..]);
            assert_close(first + c.value(), whole);
        }
        assert_eq!(
            value_of::<ReturnAutocovariance>(&prices[..2]),
            0.0,
            "one return"
        );
    }

    /// Trades at a constant mid of 100 that hit the bid at 99 or the ask at 101 at random,
    /// the model of Roll (1984).
    fn bouncing_trades(n: i64, seed: u64) -> Vec<Trade> {
        let mut state = seed;
        (0..n)
            .map(|ts| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                trade(ts, if state & 1 == 0 { 99.0 } else { 101.0 }, 1.0)
            })
            .collect()
    }

    #[test]
    fn roll_recovers_the_spread_of_a_bounce() {
        let trades = bouncing_trades(20_000, 7);
        let mut cov = ReturnAutocovariance::<TradePrice>::default();
        let mut n = ReturnCount::<TradePrice>::default();
        update_all(&mut cov, &trades);
        update_all(&mut n, &trades);
        let spread = roll_spread(n.value(), cov.value()).expect("a bounce");
        assert!((spread - 0.02).abs() < 0.001, "{spread}");
    }

    #[test]
    fn roll_needs_reverting_returns() {
        assert_eq!(roll_spread(0, 0.0), None);
        assert_eq!(roll_spread(1, -1e-4), None, "too few returns");
        assert_eq!(roll_spread(10, 0.0), None);
        assert_eq!(roll_spread(10, 1e-4), None, "trending moves");
        assert_close(roll_spread(4, -4e-4).expect("a bounce"), 0.02);
    }

    type Event = MarketEvent<Trade, Quote>;

    fn quote(timestamp: i64, bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> Event {
        Event::Quote(Quote {
            timestamp,
            bid_price: bid,
            bid_size,
            ask_price: ask,
            ask_size,
        })
    }

    fn trade_at(timestamp: i64, price: f64) -> Event {
        Event::Trade(trade(timestamp, price, 1.0))
    }

    #[test]
    fn quote_paths_ignore_the_bid_ask_bounce() {
        // The trades bounce, the quotes rise once and the microprice leans to the small queue.
        let events = [
            quote(0, 99.0, 1.0, 101.0, 1.0),
            trade_at(10, 99.0),
            trade_at(20, 101.0),
            quote(30, 100.0, 1.0, 102.0, 3.0),
            trade_at(40, 100.0),
            quote(50, 100.0, 3.0, 102.0, 1.0),
            trade_at(60, 102.0),
        ];
        fn run<C: CandleComponent + CandleComponentUpdate<Event> + Default>(
            events: &[Event],
        ) -> C::Value {
            let mut c = C::default();
            update_all(&mut c, events);
            c.close_at(80);
            c.value()
        }
        assert_eq!(run::<EfficiencyRatio<MidPrice>>(&events), Some(1.0));
        let bounce = run::<EfficiencyRatio<TradePrice>>(&events).expect("moves");
        let path = [101.0_f64 / 99.0, 101.0 / 100.0, 102.0 / 100.0].map(f64::ln);
        let net = (102.0_f64 / 99.0).ln();
        assert!((bounce - net / path.iter().sum::<f64>()).abs() < 1e-12);
        assert!(bounce < 0.7, "the bounce is not a trend");
        assert!(run::<ReturnAutocovariance<Microprice>>(&events) > 0.0);
        assert!(run::<ReturnAutocovariance<TradePrice>>(&events) < 0.0);
        let tv = run::<TotalVariation<MidPrice>>(&events);
        assert!((tv - (101.0_f64 / 100.0).ln()).abs() < 1e-15);
    }
}
