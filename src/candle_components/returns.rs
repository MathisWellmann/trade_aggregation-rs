//! Additive sums over the log returns between consecutive prices of a [`PriceSource`]:
//! the number of returns, the up, down and unchanged ticks, the signed semivariances,
//! the third and fourth powers, the total variation and the largest absolute return.
//!
//! Like [`RealizedVariance`](super::RealizedVariance), the first return of a candle is the one
//! from the last price of the previous candle. So every sum and count here is additive:
//! the sum of the values over consecutive candles equals the value over the whole series,
//! and a window of candles aggregates exactly, e.g. into the moments of
//! [`realized_skewness`] and [`realized_kurtosis`]. [`MaxAbsReturn`] is mergeable with `max`.
//! A candle without returns has a sum and a count of zero.

use crate::{
    price_source::LogReturns, CandleComponent, CandleComponentUpdate, PriceSource, TradePrice,
};

/// Defines a component that adds `$term`, a term of each log return `$r` of the candle,
/// to its value. `$check` must hold for the value `$sum` after each term.
macro_rules! return_sum {
    (
        $(#[$doc:meta])*
        $name:ident: $value:ty = |$r:ident| $term:expr, check |$sum:ident| $check:expr
    ) => {
        $(#[$doc])*
        #[derive(Debug, Default, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name<P = TradePrice> {
            sum: $value,
            returns: LogReturns<P>,
        }

        impl<P> CandleComponent for $name<P> {
            type Value = $value;

            #[inline(always)]
            fn value(&self) -> $value {
                self.sum
            }

            /// Keeps the last price, for the first return of the next candle.
            #[inline(always)]
            fn reset(&mut self) {
                self.sum = <$value>::default();
            }
        }

        impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for $name<P> {
            #[inline(always)]
            fn update(&mut self, event: &E) {
                let Some($r) = self.returns.observe(event).and_then(|step| step.log_return) else {
                    return;
                };
                self.sum += $term;
                let $sum = self.sum;
                debug_assert!($check, "{}: {} after {}", stringify!($name), $sum, $r);
            }
        }
    };
}

return_sum! {
    /// The number of log returns `n` in the candle, the count behind the sums of this module:
    /// it turns them into moments, e.g. [`realized_skewness`] and [`realized_kurtosis`],
    /// and is the number of price changes of the source (including zero changes),
    /// not the number of events. Additive.
    ///
    /// Source: the realized moments of Amaya, Christoffersen, Jacobs and Vasquez (2015).
    ReturnCount: u64 = |_r| 1, check |n| n > 0
}

return_sum! {
    /// The positive realized semivariance `RS+`: the sum of `r^2` over the log returns `r > 0`.
    /// With [`NegativeSemivariance`] it splits the [`RealizedVariance`](super::RealizedVariance)
    /// (`RS+ + RS- = RV`) into up and down moves. Additive.
    ///
    /// The signed jump variation `RS+ - RS-` predicts future volatility: negative jumps raise it,
    /// positive jumps lower it (Patton and Sheppard 2015).
    /// Source: Barndorff-Nielsen, Kinnebrock and Shephard (2010), "Measuring downside risk:
    /// realised semivariance".
    PositiveSemivariance: f64 = |r| if r > 0.0 { r * r } else { 0.0 }, check |sum| sum >= 0.0
}

return_sum! {
    /// The negative realized semivariance `RS-`: the sum of `r^2` over the log returns `r < 0`,
    /// the downside risk of the candle. See [`PositiveSemivariance`]. Additive.
    ///
    /// Source: Barndorff-Nielsen, Kinnebrock and Shephard (2010); Patton and Sheppard (2015).
    NegativeSemivariance: f64 = |r| if r < 0.0 { r * r } else { 0.0 }, check |sum| sum >= 0.0
}

return_sum! {
    /// The realized third power `sum r^3` of the log returns: the asymmetry of the moves,
    /// dominated by the largest ones. Divide it with [`realized_skewness`]
    /// to get a scale-free skewness. Additive.
    ///
    /// Source: Amaya, Christoffersen, Jacobs and Vasquez (2015), "Does realized skewness predict
    /// the cross-section of equity returns?".
    RealizedCubic: f64 = |r| r.powi(3), check |sum| sum.is_finite()
}

return_sum! {
    /// The realized fourth power `sum r^4` of the log returns: how much of the variance comes
    /// from a few large moves. Divide it with [`realized_kurtosis`] to get a scale-free kurtosis.
    /// Additive, unlike [`RealizedQuarticity`](super::RealizedQuarticity) `= n / 3 * sum r^4`,
    /// which scales the sum by the number of returns of the candle
    /// (`sum r^4 = 3 * quarticity / n` for a candle with `n > 0` returns).
    ///
    /// Source: the realized quarticity of Barndorff-Nielsen and Shephard (2002);
    /// Amaya, Christoffersen, Jacobs and Vasquez (2015).
    RealizedQuartic: f64 = |r| r.powi(4), check |sum| sum >= 0.0
}

return_sum! {
    /// The total variation `sum |r|` of the log returns, the "coastline" of the price path:
    /// the distance the log price travelled. Against the net move `|sum r|` it tells a trend
    /// from a chop (see [`EfficiencyRatio`](super::EfficiencyRatio)), and on trade prices it grows
    /// with the bid-ask bounce. Additive.
    ///
    /// Source: the realized power variation of order 1 of Barndorff-Nielsen and Shephard (2004),
    /// "Power and bipower variation with stochastic volatility and jumps".
    TotalVariation: f64 = |r| r.abs(), check |sum| sum >= 0.0
}

return_sum! {
    /// The number of up ticks, the log returns `r > 0`. The share of up ticks among the
    /// [`ReturnCount`] returns proxies buying pressure on trade prices (the tick test). Additive.
    ///
    /// Source: the tick test of Lee and Ready (1991),
    /// "Inferring trade direction from intraday data".
    UpTicks: u64 = |r| u64::from(r > 0.0), check |n| r <= 0.0 || n > 0
}

return_sum! {
    /// The number of down ticks, the log returns `r < 0`. See [`UpTicks`]. Additive.
    ///
    /// Source: the tick test of Lee and Ready (1991).
    DownTicks: u64 = |r| u64::from(r < 0.0), check |n| r >= 0.0 || n > 0
}

return_sum! {
    /// The number of unchanged ticks, the log returns `r == 0` between equal prices.
    /// Their share among the [`ReturnCount`] returns measures illiquidity: the price of an
    /// illiquid asset does not move when the information is not worth the transaction costs.
    /// Additive.
    ///
    /// Source: the zero returns measure of Lesmond, Ogden and Trzcinka (1999),
    /// "A new estimate of transaction costs".
    UnchangedTicks: u64 = |r| u64::from(r == 0.0), check |n| r != 0.0 || n > 0
}

/// The largest absolute log return `max |r|` of the candle, `None` without a return.
/// Against the realized variance it flags a jump: `max |r| / sqrt(RV)` is near 1 when one move
/// makes up the variance and near `sqrt(1 / n)` for `n` moves of equal size.
/// Mergeable: the value over consecutive candles is the `max` of their values.
///
/// Source: jump detection by the return over the local volatility,
/// Lee and Mykland (2008), "Jumps in financial markets: a new nonparametric test and jump
/// dynamics".
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxAbsReturn<P = TradePrice> {
    max: Option<f64>,
    returns: LogReturns<P>,
}

impl<P> CandleComponent for MaxAbsReturn<P> {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.max
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.max = None;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for MaxAbsReturn<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return;
        };
        let max = self.max.map_or(r.abs(), |max| max.max(r.abs()));
        debug_assert!(max >= r.abs());
        self.max = Some(max);
    }
}

/// The realized skewness `sqrt(n) * sum r^3 / RV^(3/2)` of `n` log returns, from the sums of a
/// candle or of a window of candles: [`ReturnCount`], [`RealizedVariance`](super::RealizedVariance)
/// `RV = sum r^2` and [`RealizedCubic`] `sum r^3`, all of the same price source.
/// Scale-free, in `[-sqrt(n), sqrt(n)]`; negative when the large moves are down.
/// `None` without returns or when the variance is zero.
///
/// Source: Amaya, Christoffersen, Jacobs and Vasquez (2015), "Does realized skewness predict
/// the cross-section of equity returns?".
///
/// # Panics
/// If `realized_variance` is negative or not finite, or if `cubic` is not finite.
#[must_use]
pub fn realized_skewness(n: u64, realized_variance: f64, cubic: f64) -> Option<f64> {
    assert!(
        realized_variance >= 0.0,
        "a realized variance is not negative, got {realized_variance}"
    );
    assert!(realized_variance.is_finite(), "{realized_variance}");
    assert!(cubic.is_finite(), "{cubic}");
    if n == 0 || realized_variance == 0.0 {
        return None;
    }
    let root_n = (n as f64).sqrt();
    let skewness = root_n * cubic / realized_variance.powf(1.5);
    // |sum r^3| <= max |r| * sum r^2 <= RV^(3/2), up to rounding.
    debug_assert!(skewness.abs() <= root_n * (1.0 + 1e-9), "{skewness}");
    Some(skewness)
}

/// The realized kurtosis `n * sum r^4 / RV^2` of `n` log returns, from the sums of a candle or
/// of a window of candles: [`ReturnCount`], [`RealizedVariance`](super::RealizedVariance)
/// `RV = sum r^2` and [`RealizedQuartic`] `sum r^4`, all of the same price source.
/// Scale-free, in `[1, n]`; 3 for normal returns of constant variance, larger with jumps or
/// changing volatility. `None` without returns or when the variance is zero.
///
/// Source: Amaya, Christoffersen, Jacobs and Vasquez (2015).
///
/// # Panics
/// If `realized_variance` or `quartic` is negative or not finite.
#[must_use]
pub fn realized_kurtosis(n: u64, realized_variance: f64, quartic: f64) -> Option<f64> {
    assert!(
        realized_variance >= 0.0,
        "a realized variance is not negative, got {realized_variance}"
    );
    assert!(realized_variance.is_finite(), "{realized_variance}");
    assert!(
        quartic >= 0.0,
        "a sum of r^4 is not negative, got {quartic}"
    );
    assert!(quartic.is_finite(), "{quartic}");
    if n == 0 || realized_variance == 0.0 {
        return None;
    }
    let kurtosis = n as f64 * quartic / (realized_variance * realized_variance);
    // (sum r^2)^2 / n <= sum r^4 <= (sum r^2)^2, up to rounding.
    debug_assert!(kurtosis >= 1.0 - 1e-9, "{kurtosis}");
    debug_assert!(kurtosis <= n as f64 * (1.0 + 1e-9), "{kurtosis}");
    Some(kurtosis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trades_at_prices, candle_components::RealizedVariance,
        MarketEvent, Microprice, MidPrice, Quote, Trade,
    };

    const PRICES: [f64; 5] = [100.0, 101.0, 99.0, 99.0, 102.0];

    fn returns(prices: &[f64]) -> Vec<f64> {
        prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect()
    }

    fn update_all<E, C: CandleComponentUpdate<E>>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    /// The value of a fresh component over `trades`.
    fn value_of<C>(trades: &[Trade]) -> C::Value
    where
        C: CandleComponent + CandleComponentUpdate<Trade> + Default,
    {
        let mut c = C::default();
        update_all(&mut c, trades);
        c.value()
    }

    /// Splits `trades` into two candles at every index and asserts that `merges` the values
    /// of the two candles into the value of one candle over all trades.
    fn assert_mergeable<C>(trades: &[Trade], merges: impl Fn(C::Value, C::Value, C::Value) -> bool)
    where
        C: CandleComponent + CandleComponentUpdate<Trade> + Default,
        C::Value: Copy + std::fmt::Debug,
    {
        let whole = value_of::<C>(trades);
        for at in 0..=trades.len() {
            let mut c = C::default();
            update_all(&mut c, &trades[..at]);
            let first = c.value();
            c.reset();
            update_all(&mut c, &trades[at..]);
            let second = c.value();
            assert!(
                merges(first, second, whole),
                "split at {at}: {first:?} and {second:?}, whole {whole:?}"
            );
        }
    }

    fn assert_close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-15, "{a} != {b}");
    }

    #[test]
    fn sums_over_a_short_path() {
        let trades = trades_at_prices(&PRICES);
        let r = returns(&PRICES);
        // Up, down, unchanged, up.
        assert!(r[0] > 0.0);
        assert!(r[1] < 0.0);
        assert_eq!(r[2], 0.0);
        assert!(r[3] > 0.0);
        assert_eq!(value_of::<ReturnCount>(&trades), 4);
        assert_eq!(value_of::<UpTicks>(&trades), 2);
        assert_eq!(value_of::<DownTicks>(&trades), 1);
        assert_eq!(value_of::<UnchangedTicks>(&trades), 1);
        let positive = value_of::<PositiveSemivariance>(&trades);
        let negative = value_of::<NegativeSemivariance>(&trades);
        assert_close(positive, r[0] * r[0] + r[3] * r[3]);
        assert_close(negative, r[1] * r[1]);
        assert_close(positive + negative, value_of::<RealizedVariance>(&trades));
        let sum = |f: fn(f64) -> f64| r.iter().copied().map(f).sum::<f64>();
        assert_close(value_of::<RealizedCubic>(&trades), sum(|r| r.powi(3)));
        assert_close(value_of::<RealizedQuartic>(&trades), sum(|r| r.powi(4)));
        assert_close(value_of::<TotalVariation>(&trades), sum(f64::abs));
        assert_eq!(value_of::<MaxAbsReturn>(&trades), Some(r[3]));
        assert!(r[3] > r[1].abs(), "the last return is the largest");
    }

    #[test]
    fn the_first_price_has_no_return() {
        let trades = trades_at_prices(&PRICES[..1]);
        assert_eq!(value_of::<ReturnCount>(&trades), 0);
        assert_eq!(value_of::<UnchangedTicks>(&trades), 0);
        assert_eq!(value_of::<PositiveSemivariance>(&trades), 0.0);
        assert_eq!(value_of::<TotalVariation>(&trades), 0.0);
        assert_eq!(value_of::<MaxAbsReturn>(&trades), None);
    }

    #[test]
    fn sums_are_additive_over_candle_boundaries() {
        let trades = trades_at_prices(&[100.0, 101.0, 99.0, 99.0, 102.0, 98.5, 98.5, 103.0]);
        let add_u64 = |a: u64, b: u64, whole: u64| a + b == whole;
        assert_mergeable::<ReturnCount>(&trades, add_u64);
        assert_mergeable::<UpTicks>(&trades, add_u64);
        assert_mergeable::<DownTicks>(&trades, add_u64);
        assert_mergeable::<UnchangedTicks>(&trades, add_u64);
        let add_f64 = |a: f64, b: f64, whole: f64| (a + b - whole).abs() < 1e-15;
        assert_mergeable::<PositiveSemivariance>(&trades, add_f64);
        assert_mergeable::<NegativeSemivariance>(&trades, add_f64);
        assert_mergeable::<RealizedCubic>(&trades, add_f64);
        assert_mergeable::<RealizedQuartic>(&trades, add_f64);
        assert_mergeable::<TotalVariation>(&trades, add_f64);
        assert_mergeable::<MaxAbsReturn>(&trades, |a, b, whole| {
            let max = match (a, b) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            max == whole
        });
        assert_eq!(value_of::<ReturnCount>(&trades), 7);
    }

    #[test]
    fn an_empty_candle_after_a_reset_is_zero() {
        let trades = trades_at_prices(&PRICES);
        let mut count = ReturnCount::<TradePrice>::default();
        let mut tv = TotalVariation::<TradePrice>::default();
        let mut max = MaxAbsReturn::<TradePrice>::default();
        update_all(&mut count, &trades);
        update_all(&mut tv, &trades);
        update_all(&mut max, &trades);
        assert!(tv.value() > 0.0);
        count.reset();
        tv.reset();
        max.reset();
        assert_eq!(count.value(), 0);
        assert_eq!(tv.value(), 0.0);
        assert_eq!(max.value(), None);
        // The next candle starts at the last price of the previous one, 102.
        let next = trades_at_prices(&[102.0]);
        count.update(&next[0]);
        max.update(&next[0]);
        assert_eq!(count.value(), 1);
        assert_eq!(max.value(), Some(0.0));
    }

    type Event = MarketEvent<Trade, Quote>;

    fn quote(bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> Event {
        Event::Quote(Quote {
            timestamp: 0,
            bid_price: bid,
            bid_size,
            ask_price: ask,
            ask_size,
        })
    }

    fn trade(price: f64) -> Event {
        Event::Trade(Trade {
            timestamp: 0,
            price,
            size: 1.0,
        })
    }

    /// The return count, up, down and unchanged ticks of `P` over `events`.
    fn ticks<P: PriceSource<Event> + Default>(events: &[Event]) -> [u64; 4] {
        let (mut n, mut up, mut down, mut flat) = (
            ReturnCount::<P>::default(),
            UpTicks::<P>::default(),
            DownTicks::<P>::default(),
            UnchangedTicks::<P>::default(),
        );
        update_all(&mut n, events);
        update_all(&mut up, events);
        update_all(&mut down, events);
        update_all(&mut flat, events);
        assert_eq!(n.value(), up.value() + down.value() + flat.value());
        [n.value(), up.value(), down.value(), flat.value()]
    }

    #[test]
    fn quote_prices_ignore_the_bid_ask_bounce() {
        // Trades bounce between the bid and the ask while the quotes move up once.
        let events = [
            quote(99.0, 1.0, 101.0, 3.0),
            trade(99.0),
            trade(101.0),
            trade(99.0),
            quote(100.0, 1.0, 102.0, 3.0),
            trade(102.0),
            trade(100.0),
            quote(100.0, 3.0, 102.0, 1.0),
        ];
        // 99 101 99 102 100: up, down, up, down.
        assert_eq!(ticks::<TradePrice>(&events), [4, 2, 2, 0]);
        // Mid 100, 101, 101.
        assert_eq!(ticks::<MidPrice>(&events), [2, 1, 0, 1]);
        // The microprice leans towards the smaller queue: 99.5, 100.5, 101.5.
        assert_eq!(ticks::<Microprice>(&events), [2, 2, 0, 0]);

        let mut micro = TotalVariation::<Microprice>::default();
        update_all(&mut micro, &events);
        assert_close(micro.value(), (101.5_f64 / 99.5).ln());
        let mut mid = MaxAbsReturn::<MidPrice>::default();
        update_all(&mut mid, &events);
        assert_eq!(mid.value(), Some((101.0_f64 / 100.0).ln()));
        let mut trades = TotalVariation::<TradePrice>::default();
        update_all(&mut trades, &events);
        assert!(
            trades.value() > 4.0 * micro.value(),
            "the bounce adds to the trade path"
        );
    }

    #[test]
    fn skewness_and_kurtosis_of_a_short_path() {
        let r = returns(&PRICES);
        let sum = |p: i32| r.iter().map(|r| r.powi(p)).sum::<f64>();
        let n = r.len() as u64;
        let skewness = realized_skewness(n, sum(2), sum(3)).expect("returns");
        assert_close(skewness, 2.0 * sum(3) / sum(2).powf(1.5));
        assert!(skewness > 0.0, "the largest move is up");
        let kurtosis = realized_kurtosis(n, sum(2), sum(4)).expect("returns");
        assert_close(kurtosis, 4.0 * sum(4) / (sum(2) * sum(2)));
        assert!(kurtosis > 1.0);
        assert!(kurtosis < 4.0);
    }

    #[test]
    fn skewness_and_kurtosis_at_the_edges() {
        // Symmetric moves have no skewness; equal moves have the minimal kurtosis.
        let r = 0.01_f64;
        assert_eq!(realized_skewness(2, 2.0 * r * r, 0.0), Some(0.0));
        assert_close(
            realized_kurtosis(2, 2.0 * r * r, 2.0 * r.powi(4)).expect("returns"),
            1.0,
        );
        // One move has the extreme skewness sqrt(1) and kurtosis 1.
        assert_close(
            realized_skewness(1, r * r, -r.powi(3)).expect("a return"),
            -1.0,
        );
        // Without returns, or without variance, there are no moments.
        assert_eq!(realized_skewness(0, 0.0, 0.0), None);
        assert_eq!(realized_kurtosis(0, 0.0, 0.0), None);
        assert_eq!(realized_skewness(3, 0.0, 0.0), None);
        assert_eq!(realized_kurtosis(3, 0.0, 0.0), None);
    }

    #[test]
    #[should_panic(expected = "a realized variance is not negative")]
    fn a_negative_variance_has_no_skewness() {
        let _ = realized_skewness(2, -1e-6, 0.0);
    }

    #[test]
    #[should_panic(expected = "a sum of r^4 is not negative")]
    fn a_negative_quartic_has_no_kurtosis() {
        let _ = realized_kurtosis(2, 1e-4, -1e-8);
    }
}
