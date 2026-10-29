//! The parts shared by the order flow bars of López de Prado
//! (Advances in Financial Machine Learning, ch. 2.3.2):
//! [`ImbalanceRule`](crate::ImbalanceRule) and the run bars that build on the same parts.

use crate::{By, Error, Result, TakerTrade};

/// What a trade contributes to the order flow of imbalance and run bars.
///
/// The side of a trade is the sign of [`TakerTrade::size`], the side of the taker.
/// This is exact, unlike the tick rule of the book, which infers the side from price changes.
#[derive(Debug, Clone, Copy)]
pub enum FlowSignal {
    /// Every trade counts 1: tick imbalance and tick run bars.
    Ticks,
    /// Every trade counts its size: volume bars with `By::Base`
    /// and, for sizes in quote currency, dollar bars with `By::Quote`, as for the [`VolumeRule`](crate::VolumeRule).
    Volume(By),
}

/// The side and the magnitude of one trade.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Flow {
    /// `+1` for a taker buy, `-1` for a taker sell, `0` for a trade without size.
    pub(crate) side: f64,
    /// The non-negative contribution of the trade.
    pub(crate) magnitude: f64,
}

impl FlowSignal {
    pub(crate) fn flow<T: TakerTrade>(self, trade: &T) -> Flow {
        let size = trade.size();
        debug_assert!(size.is_finite());
        debug_assert!(trade.price() > 0.0, "prices must be positive");
        let side = if size > 0.0 {
            1.0
        } else if size < 0.0 {
            -1.0
        } else {
            0.0
        };
        let magnitude = match self {
            Self::Ticks => 1.0,
            Self::Volume(By::Quote) => size.abs(),
            Self::Volume(By::Base) => size.abs() / trade.price(),
        };
        debug_assert!(magnitude >= 0.0);
        Flow { side, magnitude }
    }
}

/// How the expectations of imbalance and run bars are learned.
///
/// Both rules close a bar when the flow of the bar exceeds
/// `expected ticks per bar * expected flow per trade`.
/// The expected flow per trade is an exponentially weighted moving average over trades.
/// The expected ticks per bar is fixed by default, as in the "const" variant of mlfinlab.
/// [`Self::with_bar_alpha`] learns it from the closed bars instead, as in the book;
/// that feedback is unstable (bars tend to collapse to `min_ticks` or grow to `max_ticks`),
/// so it needs bounds.
/// To keep bars at a target duration, leave it fixed and wrap the rule in
/// [`Adaptive`](crate::Adaptive), which calibrates the expected ticks per bar.
#[derive(Debug, Clone, Copy)]
pub struct FlowBarParams {
    expected_ticks: f64,
    bar_alpha: f64,
    min_ticks: f64,
    max_ticks: f64,
    trade_alpha: f64,
    min_flow_fraction: f64,
}

impl FlowBarParams {
    /// Parameters with a fixed expected bar length of `expected_ticks` trades.
    ///
    /// The expected flow per trade averages over about ten bars
    /// (`trade_alpha = 1 / (10 * expected_ticks)`), and the expected imbalance per trade
    /// is at least 5% of the average flow per trade (see [`Self::with_min_flow_fraction`]).
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `expected_ticks` is not finite and at least 1.
    pub fn new(expected_ticks: f64) -> Result<Self> {
        check_ticks(expected_ticks)?;
        Ok(Self {
            expected_ticks,
            bar_alpha: 0.0,
            min_ticks: expected_ticks,
            max_ticks: expected_ticks,
            trade_alpha: 1.0 / (10.0 * expected_ticks),
            min_flow_fraction: 0.05,
        })
    }

    /// Learn the expected bar length from the closed bars: after each bar it moves by
    /// `bar_alpha` towards the length of that bar, clamped to `[min_ticks, max_ticks]`.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `bar_alpha` is not in `(0, 1]`, if a bound is not finite
    /// and at least 1, or if the expected bar length of [`Self::new`] is outside the bounds.
    pub fn with_bar_alpha(self, bar_alpha: f64, min_ticks: f64, max_ticks: f64) -> Result<Self> {
        check_alpha(bar_alpha)?;
        check_ticks(min_ticks)?;
        check_ticks(max_ticks)?;
        if !(min_ticks <= self.expected_ticks && self.expected_ticks <= max_ticks) {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            bar_alpha,
            min_ticks,
            max_ticks,
            ..self
        })
    }

    /// The weight of each trade in the moving averages of the flow per trade.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `trade_alpha` is not in `(0, 1]`.
    pub fn with_trade_alpha(self, trade_alpha: f64) -> Result<Self> {
        check_alpha(trade_alpha)?;
        Ok(Self {
            trade_alpha,
            ..self
        })
    }

    /// The smallest expected imbalance per trade of an [`ImbalanceRule`](crate::ImbalanceRule),
    /// as a fraction of the average flow per trade.
    ///
    /// Without a floor, balanced flow drives the expected imbalance and thus the threshold to zero,
    /// and every trade closes a bar. With the floor, a balanced market produces bars of about
    /// `(min_flow_fraction * expected_ticks)^2` trades, the time a random walk needs to reach the threshold.
    /// Run bars expect at least half the average flow anyway and ignore it.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `min_flow_fraction` is not in `(0, 1]`.
    pub fn with_min_flow_fraction(self, min_flow_fraction: f64) -> Result<Self> {
        check_alpha(min_flow_fraction)?;
        Ok(Self {
            min_flow_fraction,
            ..self
        })
    }

    pub(crate) fn expected_ticks(&self) -> f64 {
        self.expected_ticks
    }

    pub(crate) fn trade_alpha(&self) -> f64 {
        self.trade_alpha
    }

    pub(crate) fn min_flow_fraction(&self) -> f64 {
        self.min_flow_fraction
    }

    /// The expected bar length after a bar of `ticks` trades closed.
    pub(crate) fn next_expected_ticks(&self, expected_ticks: f64, ticks: f64) -> f64 {
        debug_assert!(ticks >= 1.0);
        if self.bar_alpha == 0.0 {
            return expected_ticks;
        }
        let next = (self.bar_alpha * ticks + (1.0 - self.bar_alpha) * expected_ticks)
            .clamp(self.min_ticks, self.max_ticks);
        debug_assert!(next >= 1.0);
        next
    }
}

/// An expected bar length must be finite and at least one trade.
pub(crate) fn check_ticks(ticks: f64) -> Result<()> {
    if ticks.is_finite() && ticks >= 1.0 {
        Ok(())
    } else {
        Err(Error::InvalidParam)
    }
}

fn check_alpha(alpha: f64) -> Result<()> {
    if alpha > 0.0 && alpha <= 1.0 {
        Ok(())
    } else {
        Err(Error::InvalidParam)
    }
}

/// An exponentially weighted moving average, starting at the first value.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ewma {
    alpha: f64,
    value: Option<f64>,
}

impl Ewma {
    pub(crate) fn new(alpha: f64) -> Self {
        debug_assert!(alpha > 0.0 && alpha <= 1.0);
        Self { alpha, value: None }
    }

    pub(crate) fn update(&mut self, x: f64) {
        debug_assert!(x.is_finite());
        self.value = Some(match self.value {
            None => x,
            Some(prev) => self.alpha * x + (1.0 - self.alpha) * prev,
        });
    }

    /// The average, or 0 before the first value.
    pub(crate) fn get(&self) -> f64 {
        self.value.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregation_rules::test_utils::trade;

    #[test]
    fn params_validate() {
        assert!(FlowBarParams::new(0.5).is_err());
        assert!(FlowBarParams::new(f64::NAN).is_err());
        assert!(FlowBarParams::new(f64::INFINITY).is_err());
        let p = FlowBarParams::new(100.0).expect("valid");
        assert_eq!(p.trade_alpha(), 0.001);
        assert!(p.with_trade_alpha(0.0).is_err());
        assert!(p.with_trade_alpha(1.5).is_err());
        assert!(p.with_trade_alpha(1.0).is_ok());
        assert!(p.with_min_flow_fraction(0.0).is_err());
        assert!(p.with_bar_alpha(0.0, 10.0, 1_000.0).is_err());
        assert!(p.with_bar_alpha(0.1, 0.5, 1_000.0).is_err());
        assert!(p.with_bar_alpha(0.1, 200.0, 1_000.0).is_err());
        assert!(p.with_bar_alpha(0.1, 10.0, 50.0).is_err());
        assert!(p.with_bar_alpha(0.1, 10.0, 1_000.0).is_ok());
    }

    #[test]
    fn expected_ticks_are_fixed_by_default() {
        let p = FlowBarParams::new(100.0).expect("valid");
        assert_eq!(p.next_expected_ticks(100.0, 3.0), 100.0);
    }

    #[test]
    fn expected_ticks_learn_within_bounds() {
        let p = FlowBarParams::new(100.0)
            .and_then(|p| p.with_bar_alpha(0.5, 60.0, 150.0))
            .expect("valid");
        assert_eq!(p.next_expected_ticks(100.0, 80.0), 90.0);
        assert_eq!(p.next_expected_ticks(100.0, 1.0), 60.0);
        assert_eq!(p.next_expected_ticks(100.0, 1_000.0), 150.0);
    }

    #[test]
    fn flow_of_a_trade() {
        let buy = trade(0, 50.0, 100.0);
        let sell = trade(0, 50.0, -100.0);
        let empty = trade(0, 50.0, 0.0);
        let f = FlowSignal::Ticks.flow(&sell);
        assert_eq!((f.side, f.magnitude), (-1.0, 1.0));
        let f = FlowSignal::Volume(By::Quote).flow(&buy);
        assert_eq!((f.side, f.magnitude), (1.0, 100.0));
        let f = FlowSignal::Volume(By::Base).flow(&sell);
        assert_eq!((f.side, f.magnitude), (-1.0, 2.0));
        let f = FlowSignal::Volume(By::Quote).flow(&empty);
        assert_eq!((f.side, f.magnitude), (0.0, 0.0));
    }

    #[test]
    fn ewma_starts_at_the_first_value() {
        let mut e = Ewma::new(0.5);
        assert_eq!(e.get(), 0.0);
        e.update(4.0);
        assert_eq!(e.get(), 4.0);
        e.update(0.0);
        assert_eq!(e.get(), 2.0);
    }
}
