use crate::{AggregationRule, Error, Result, TakerTrade, ThresholdRule};

/// The state of the price path, in the terms of the directional change framework.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Trend {
    /// No trade seen yet.
    Start,
    /// No directional change confirmed yet; the extremes since the first trade.
    Unknown { high: f64, low: f64 },
    /// The last confirmed directional change was upwards; the highest price since.
    Up { extreme: f64 },
    /// The last confirmed directional change was downwards; the lowest price since.
    Down { extreme: f64 },
}

/// Closes a candle at every directional change of the price, as in the "intrinsic time" of
/// Guillaume et al. (1997) and Glattfelder, Dupuis and Olsen (2011).
///
/// In an uptrend the rule tracks the highest price. A directional change is confirmed when the
/// price falls `threshold_fraction` below that extreme; then the trend is down and the rule
/// tracks the lowest price, until the price rises `threshold_fraction` above it.
/// Before the first confirmation the rule watches both directions from the first trade.
///
/// The extreme of a trend is only known in hindsight, so candles split at the confirmations:
/// each candle holds the overshoot of one trend (the move beyond the confirmation, on average
/// about `threshold_fraction` long according to the scaling laws) and the reversal into the next.
/// The state follows the price path rather than the candle, so [`reset`](AggregationRule::reset)
/// keeps it.
#[derive(Debug, Clone)]
pub struct DirectionalChangeRule {
    threshold_fraction: f64,
    trend: Trend,
}

impl DirectionalChangeRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold_fraction`: The reversal from the extreme that confirms a directional change,
    ///     e.g. 0.01 for 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold_fraction` is not in `(0, 1)`.
    pub fn new(threshold_fraction: f64) -> Result<Self> {
        let mut rule = Self {
            threshold_fraction: 0.5,
            trend: Trend::Start,
        };
        rule.set_threshold(threshold_fraction)?;
        Ok(rule)
    }
}

/// The trend after `price`, and whether `price` confirms a directional change.
fn next_trend(trend: Trend, price: f64, threshold: f64) -> (Trend, bool) {
    let up = |low: f64| price >= low * (1.0 + threshold);
    let down = |high: f64| price <= high * (1.0 - threshold);
    match trend {
        Trend::Start => (
            Trend::Unknown {
                high: price,
                low: price,
            },
            false,
        ),
        Trend::Unknown { high, low } => {
            let (high, low) = (high.max(price), low.min(price));
            if up(low) {
                (Trend::Up { extreme: price }, true)
            } else if down(high) {
                (Trend::Down { extreme: price }, true)
            } else {
                (Trend::Unknown { high, low }, false)
            }
        }
        Trend::Up { extreme } if down(extreme) => (Trend::Down { extreme: price }, true),
        Trend::Up { extreme } => (
            Trend::Up {
                extreme: extreme.max(price),
            },
            false,
        ),
        Trend::Down { extreme } if up(extreme) => (Trend::Up { extreme: price }, true),
        Trend::Down { extreme } => (
            Trend::Down {
                extreme: extreme.min(price),
            },
            false,
        ),
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for DirectionalChangeRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        let price = trade.price();
        debug_assert!(price > 0.0, "prices must be positive");
        let (trend, confirmed) = next_trend(self.trend, price, self.threshold_fraction);
        if confirmed {
            debug_assert!(matches!(trend, Trend::Up { .. } | Trend::Down { .. }));
        }
        self.trend = trend;
        confirmed
    }
}

impl ThresholdRule for DirectionalChangeRule {
    fn threshold(&self) -> f64 {
        self.threshold_fraction
    }

    /// A threshold of 1 or more could never confirm a downward change.
    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !(threshold > 0.0 && threshold < 1.0) {
            return Err(Error::InvalidParam);
        }
        self.threshold_fraction = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trades_at_prices, trigger_indices},
        load_trades_from_csv,
        plot::OhlcCandle,
        Trade,
    };

    fn rule(threshold: f64) -> DirectionalChangeRule {
        DirectionalChangeRule::new(threshold).expect("valid threshold")
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(DirectionalChangeRule::new(0.0).is_err());
        assert!(DirectionalChangeRule::new(1.0).is_err());
        assert!(DirectionalChangeRule::new(-0.1).is_err());
        assert!(DirectionalChangeRule::new(f64::NAN).is_err());
        assert!(DirectionalChangeRule::new(0.999).is_ok());
    }

    #[test]
    fn triggers_at_each_confirmed_reversal() {
        let trades = trades_at_prices(&[
            100.0, 100.5, 99.5, // Unknown: high 100.5, low 99.5, no move of 10% yet
            90.0, // 10.4% below the high of 100.5: down
            85.0, 88.0, // new low 85, 3.5% above it
            93.6, // more than 10% above the low of 85: up
            95.0, 99.0, 90.0, // high 99, 9.1% below it
            89.0, // more than 10% below the high of 99: down
        ]);
        assert_eq!(trigger_indices(&mut rule(0.1), &trades), vec![3, 6, 10]);
    }

    #[test]
    fn the_first_change_can_go_up() {
        let trades = trades_at_prices(&[100.0, 98.0, 107.9, 108.0]);
        // 107.9 is 10% above the low of 98, but not 10% above the first price.
        assert_eq!(trigger_indices(&mut rule(0.1), &trades), vec![2]);
    }

    #[test]
    fn a_trend_without_reversal_does_not_trigger() {
        let prices: Vec<f64> = (0..100).map(|i| 100.0 * 1.01_f64.powi(i)).collect();
        let mut r = rule(0.05);
        assert_eq!(trigger_indices(&mut r, &trades_at_prices(&prices)), vec![5]);
        // Only the first move from the start confirms a change; the uptrend then continues.
        assert!(matches!(r.trend, Trend::Up { extreme } if extreme == prices[99]));
    }

    #[test]
    fn reset_keeps_the_trend() {
        let mut r = rule(0.1);
        assert_eq!(
            trigger_indices(&mut r, &trades_at_prices(&[100.0, 111.0])),
            vec![1]
        );
        AggregationRule::<OhlcCandle, Trade>::reset(&mut r);
        assert_eq!(r.trend, Trend::Up { extreme: 111.0 });
        assert_eq!(
            trigger_indices(&mut r, &trades_at_prices(&[105.0, 99.8])),
            vec![1]
        );
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(0.1);
        r.set_threshold(0.2).expect("valid threshold");
        assert_eq!(r.threshold(), 0.2);
        assert!(r.set_threshold(1.5).is_err());
        assert_eq!(r.threshold(), 0.2);
    }

    #[test]
    fn larger_thresholds_confirm_fewer_changes() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("test data");
        let counts: Vec<usize> = [0.001, 0.002, 0.005, 0.01]
            .iter()
            .map(|threshold| trigger_indices(&mut rule(*threshold), &trades).len())
            .collect();
        assert!(counts[3] > 0, "counts {counts:?}");
        for pair in counts.windows(2) {
            assert!(pair[0] > pair[1], "counts {counts:?}");
        }
    }
}
