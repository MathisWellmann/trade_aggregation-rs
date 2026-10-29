use crate::{AggregationRule, Error, Result, TakerTrade, ThresholdRule};

/// Closes a candle when a symmetric CUSUM filter on the log returns of the trades fires
/// (Lam and Yam 1997; López de Prado, Advances in Financial Machine Learning, ch. 2.5.2.1).
///
/// With `r` the log return from the previous trade, the rule keeps
/// `s_up = max(0, s_up + r)` and `s_down = min(0, s_down + r)`.
/// It triggers when `s_up >= threshold` or `-s_down >= threshold`, and then restarts both sums.
/// Unlike a [`RelativePriceRule`](crate::RelativePriceRule), small moves against the run
/// do not restart it, and noise around a level does not add up as in a volatility rule:
/// it samples when the price has drifted in one direction.
#[derive(Debug, Clone)]
pub struct CusumRule {
    threshold: f64,
    previous_price: Option<f64>,
    s_up: f64,
    s_down: f64,
}

impl CusumRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold`: The cumulative log return at which the filter fires, e.g. 0.01 for about 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64) -> Result<Self> {
        let mut rule = Self {
            threshold: 1.0,
            previous_price: None,
            s_up: 0.0,
            s_down: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for CusumRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        let price = trade.price();
        debug_assert!(price > 0.0, "prices must be positive");
        let Some(previous_price) = self.previous_price.replace(price) else {
            return false;
        };
        let log_return = (price / previous_price).ln();
        self.s_up = (self.s_up + log_return).max(0.0);
        self.s_down = (self.s_down + log_return).min(0.0);
        debug_assert!(self.s_up >= 0.0);
        debug_assert!(self.s_down <= 0.0);
        let trigger = self.s_up >= self.threshold || -self.s_down >= self.threshold;
        if trigger {
            self.s_up = 0.0;
            self.s_down = 0.0;
        }
        trigger
    }

    /// Restart both sums; the last price stays the reference for the next return.
    fn reset(&mut self) {
        self.s_up = 0.0;
        self.s_down = 0.0;
    }
}

impl ThresholdRule for CusumRule {
    fn threshold(&self) -> f64 {
        self.threshold
    }

    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        self.threshold = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trades_at_prices, trigger_indices},
        plot::OhlcCandle,
        Trade,
    };

    /// Prices that move by the given log returns from 100.
    fn prices_from_returns(returns: &[f64]) -> Vec<f64> {
        let mut price = 100.0;
        let mut prices = vec![price];
        for r in returns {
            price *= r.exp();
            prices.push(price);
        }
        prices
    }

    fn rule(threshold: f64) -> CusumRule {
        CusumRule::new(threshold).expect("valid threshold")
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(CusumRule::new(0.0).is_err());
        assert!(CusumRule::new(-0.01).is_err());
        assert!(CusumRule::new(f64::NAN).is_err());
        assert!(CusumRule::new(f64::INFINITY).is_err());
    }

    #[test]
    fn fires_on_a_drift_despite_small_pullbacks() {
        // Up 0.4%, down 0.1%, repeated: the up sum grows by 0.3% per pair.
        let returns = [0.004, -0.001, 0.004, -0.001, 0.004, -0.001, 0.004];
        let trades = trades_at_prices(&prices_from_returns(&returns));
        // s_up after each trade: 0.4, 0.3, 0.7, 0.6, 1.0 (fires on trade 5), ...
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), vec![5]);
    }

    #[test]
    fn fires_downwards() {
        let trades = trades_at_prices(&prices_from_returns(&[-0.006, 0.001, -0.006]));
        assert_eq!(trigger_indices(&mut rule(0.01), &trades), vec![3]);
    }

    #[test]
    fn noise_around_a_level_does_not_fire() {
        let returns: Vec<f64> = (0..1_000)
            .map(|i| if i % 2 == 0 { 0.005 } else { -0.005 })
            .collect();
        let trades = trades_at_prices(&prices_from_returns(&returns));
        assert!(trigger_indices(&mut rule(0.01), &trades).is_empty());
    }

    #[test]
    fn both_sums_restart_after_firing() {
        // Down 0.8% first, then up 1%: fires on the way up and restarts s_down,
        // so the next 0.3% down does not complete the earlier 0.8%.
        let trades = trades_at_prices(&prices_from_returns(&[-0.008, 0.01, -0.003]));
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), vec![2]);
    }

    #[test]
    fn reset_restarts_the_sums_but_keeps_the_reference_price() {
        let mut r = rule(0.00999);
        let prices = prices_from_returns(&[0.008, 0.002, 0.008, 0.002]);
        let trades = trades_at_prices(&prices);
        assert!(trigger_indices(&mut r, &trades[..2]).is_empty());
        AggregationRule::<OhlcCandle, Trade>::reset(&mut r);
        // From trade 1 on: 0.2% (trade 2), 1.0% (trade 3).
        assert_eq!(trigger_indices(&mut r, &trades[2..]), vec![1]);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(0.01);
        r.set_threshold(0.02).expect("valid threshold");
        assert_eq!(r.threshold(), 0.02);
        assert!(r.set_threshold(-1.0).is_err());
        assert_eq!(r.threshold(), 0.02);
    }
}
