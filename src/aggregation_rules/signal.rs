//! The values that a [`CusumRule`](crate::CusumRule) accumulates, read from each event,
//! and their standardization to units of standard deviations.

use crate::{price_source::LogReturns, Error, PriceSource, Result, TradePrice};

/// A value read from each event of a stream, e.g. the log return of a price,
/// that a rule such as the [`CusumRule`](crate::CusumRule) accumulates.
///
/// A signal follows the stream, not the candle: it keeps its state (e.g. the previous price
/// or its moving averages) when a candle closes, and rules do not reset it.
/// It may only use the events up to the current one, so that a candle never depends
/// on a later event (no lookahead).
pub trait Signal<E> {
    /// The value of the signal at `event`, `None` if the event carries no value of this signal,
    /// e.g. a quote for a signal of the trades, or the first price for a return.
    fn observe(&mut self, event: &E) -> Option<f64>;
}

/// The log return `ln(p / p_previous)` between consecutive prices of a [`PriceSource`],
/// the trade prices by default.
///
/// The first price has no return. The last price stays the reference when a candle closes,
/// so no return is lost at a candle boundary.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogReturn<P = TradePrice> {
    returns: LogReturns<P>,
}

impl<P> LogReturn<P> {
    /// Follow the prices of `source`, e.g. [`TradePrice`], [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice).
    pub fn new(source: P) -> Self {
        Self {
            returns: LogReturns::new(source),
        }
    }
}

impl<E, P: PriceSource<E>> Signal<E> for LogReturn<P> {
    #[inline(always)]
    fn observe(&mut self, event: &E) -> Option<f64> {
        self.returns.observe(event)?.log_return
    }
}

/// The z-score `(x - mean) / standard deviation` of an inner signal, with an exponentially
/// weighted moving mean and variance (the incremental form of Finch 2009),
/// so that its values are in units of standard deviations of the recent past.
///
/// A threshold on standardized values means the same for every symbol and every regime,
/// e.g. the threshold of a [`CusumRule`](crate::CusumRule) becomes a number of standard deviations.
/// Removing the mean also keeps a signal with a drift from adding up by itself.
///
/// There is no lookahead: each value is standardized with the mean and the variance
/// of the values before it, and only then updates them.
/// The first `warmup` values only feed the statistics and give `None`.
/// As long as `1 / n > alpha` for the `n`-th value, the statistics weigh every value equally
/// (the plain mean and the population variance), so that a small `alpha` does not start
/// from a variance near zero, which would give huge first z-scores.
/// While the variance is zero (e.g. a constant signal), the scale is unknown and the value
/// gives `None`; so does a z-score that overflows.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Standardized<S> {
    inner: S,
    warmup: u64,
    stats: EwStats,
}

impl<S> Standardized<S> {
    /// Standardize the values of `inner`.
    ///
    /// # Arguments:
    /// `inner`: The signal to standardize, e.g. a [`LogReturn`].
    /// `alpha`: The weight of the newest value in the moving mean and variance, in `(0, 1)`;
    ///     about `2 / (n + 1)` for a memory of `n` values.
    /// `warmup`: The number of values that only feed the statistics, at least 2.
    ///     About `1 / alpha` makes the first z-scores as reliable as the later ones.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `alpha` is not in `(0, 1)` or `warmup` is less than 2.
    /// With `alpha = 1` the variance would always be zero, so no value would ever be standardized.
    pub fn new(inner: S, alpha: f64, warmup: u64) -> Result<Self> {
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(Error::InvalidParam);
        }
        if warmup < 2 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            inner,
            warmup,
            stats: EwStats::new(alpha),
        })
    }
}

impl<E, S: Signal<E>> Signal<E> for Standardized<S> {
    fn observe(&mut self, event: &E) -> Option<f64> {
        let x = self.inner.observe(event)?;
        assert!(x.is_finite(), "signal values must be finite, got {x}");
        let z = if self.stats.count >= self.warmup {
            self.stats.z_score(x)
        } else {
            None
        };
        self.stats.update(x);
        debug_assert!(self.stats.count > 0);
        z
    }
}

/// An exponentially weighted moving mean and variance whose weight is at least `1 / n`
/// for the `n`-th value, so that it starts as the plain mean and population variance.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct EwStats {
    alpha: f64,
    count: u64,
    mean: f64,
    variance: f64,
}

impl EwStats {
    fn new(alpha: f64) -> Self {
        assert!(alpha > 0.0, "alpha must be positive");
        assert!(alpha < 1.0, "alpha must be below 1");
        Self {
            alpha,
            count: 0,
            mean: 0.0,
            variance: 0.0,
        }
    }

    /// The z-score of `x` against the values so far, `None` if their variance is zero.
    fn z_score(&self, x: f64) -> Option<f64> {
        debug_assert!(self.count > 0, "the statistics need a value");
        debug_assert!(self.variance >= 0.0);
        if self.variance <= 0.0 {
            return None;
        }
        let z = (x - self.mean) / self.variance.sqrt();
        z.is_finite().then_some(z)
    }

    fn update(&mut self, x: f64) {
        debug_assert!(x.is_finite());
        self.count = self.count.saturating_add(1);
        // The weight only matters while `1 / n > alpha`, i.e. for small counts.
        let n = f64::from(u32::try_from(self.count).unwrap_or(u32::MAX));
        let weight = self.alpha.max(1.0 / n);
        let diff = x - self.mean;
        let increment = weight * diff;
        self.mean += increment;
        self.variance = (1.0 - weight) * (self.variance + diff * increment);
        debug_assert!(self.variance >= 0.0, "a sum of non-negative terms");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarketEvent, MidPrice, Quote, Trade};

    /// A signal whose events are its values.
    #[derive(Debug, Clone)]
    struct Values;

    impl Signal<f64> for Values {
        fn observe(&mut self, event: &f64) -> Option<f64> {
            Some(*event)
        }
    }

    fn standardized(alpha: f64, warmup: u64) -> Standardized<Values> {
        Standardized::new(Values, alpha, warmup).expect("valid parameters")
    }

    fn observe_all<S: Signal<f64>>(signal: &mut S, values: &[f64]) -> Vec<Option<f64>> {
        values.iter().map(|v| signal.observe(v)).collect()
    }

    #[test]
    fn log_returns_of_the_trades() {
        let mut r: LogReturn = LogReturn::default();
        let trade = |price| Trade {
            timestamp: 0,
            price,
            size: 1.0,
        };
        assert_eq!(r.observe(&trade(100.0)), None);
        assert_eq!(r.observe(&trade(110.0)), Some((1.1_f64).ln()));
        assert_eq!(r.observe(&trade(110.0)), Some(0.0));
    }

    #[test]
    fn log_returns_of_the_mid_price_skip_trades() {
        let mut r = LogReturn::new(MidPrice);
        let quote = |bid, ask| {
            MarketEvent::<Trade, Quote>::Quote(Quote {
                timestamp: 0,
                bid_price: bid,
                bid_size: 1.0,
                ask_price: ask,
                ask_size: 1.0,
            })
        };
        let trade = MarketEvent::<Trade, Quote>::Trade(Trade {
            timestamp: 0,
            price: 500.0,
            size: 1.0,
        });
        assert_eq!(r.observe(&quote(99.0, 101.0)), None);
        assert_eq!(r.observe(&trade), None);
        assert_eq!(r.observe(&quote(0.0, 101.0)), None, "an invalid quote");
        assert_eq!(r.observe(&quote(109.0, 111.0)), Some((1.1_f64).ln()));
    }

    #[test]
    fn standardized_rejects_invalid_parameters() {
        assert!(Standardized::new(Values, 0.0, 10).is_err());
        assert!(Standardized::new(Values, -0.1, 10).is_err());
        assert!(Standardized::new(Values, 1.0, 10).is_err());
        assert!(Standardized::new(Values, f64::NAN, 10).is_err());
        assert!(Standardized::new(Values, 0.1, 0).is_err());
        assert!(Standardized::new(Values, 0.1, 1).is_err());
        assert!(Standardized::new(Values, 0.1, 2).is_ok());
    }

    #[test]
    fn standardized_uses_only_the_values_before() {
        // 1 and 3 warm up: mean 2, population variance 1.
        // 4 is 2 standard deviations above; then (alpha 0.5) mean 3, variance 0.5 * (1 + 2 * 1) = 1.5.
        // 3 is at the mean.
        let z = observe_all(&mut standardized(0.5, 2), &[1.0, 3.0, 4.0, 3.0]);
        assert_eq!(z, vec![None, None, Some(2.0), Some(0.0)]);
    }

    #[test]
    fn standardized_starts_with_equal_weights() {
        // With alpha 0.01, the first values weigh 1/n: after 1, 2, 3 the mean is 2
        // and the population variance 2/3, as without exponential weights.
        let mut s = standardized(0.01, 3);
        let z = observe_all(&mut s, &[1.0, 2.0, 3.0, 2.0 + (2.0_f64 / 3.0).sqrt()]);
        assert_eq!(z[..3], [None, None, None]);
        let last = z[3].expect("after the warm-up");
        assert!((last - 1.0).abs() < 1e-12, "{last}");
    }

    #[test]
    fn standardized_skips_a_zero_variance() {
        let mut s = standardized(0.5, 2);
        // Constant values: the scale is unknown, even for a different value.
        assert_eq!(observe_all(&mut s, &[5.0, 5.0, 5.0, 7.0]), vec![None; 4]);
        // 7 updated the statistics: mean 6, variance 0.5 * (0 + 2 * 1) = 1.
        assert_eq!(s.observe(&6.0), Some(0.0));
    }

    #[test]
    #[should_panic(expected = "finite")]
    fn standardized_rejects_a_non_finite_value() {
        let _ = observe_all(&mut standardized(0.5, 2), &[1.0, f64::NAN]);
    }

    #[test]
    fn standardized_values_do_not_depend_on_the_scale() {
        let values: Vec<f64> = (0..200).map(|i| f64::from(i % 7) - 2.5).collect();
        let scaled: Vec<f64> = values.iter().map(|v| 1_000.0 * v).collect();
        let a = observe_all(&mut standardized(0.05, 20), &values);
        let b = observe_all(&mut standardized(0.05, 20), &scaled);
        assert_eq!(a.iter().filter(|z| z.is_some()).count(), 180);
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                assert!((a - b).abs() < 1e-9, "{a} != {b}");
            }
        }
    }

    #[test]
    fn standardized_passes_events_without_a_value() {
        let mut s = Standardized::new(LogReturn::new(MidPrice), 0.5, 2).expect("valid parameters");
        let trade = MarketEvent::<Trade, Quote>::Trade(Trade {
            timestamp: 0,
            price: 100.0,
            size: 1.0,
        });
        for _ in 0..10 {
            assert_eq!(s.observe(&trade), None);
        }
        assert_eq!(s.stats.count, 0, "events without a value do not count");
    }
}
