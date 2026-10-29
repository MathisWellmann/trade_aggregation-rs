use crate::{AggregationRule, Error, Result, TakerTrade, ThresholdRule};

/// Creates volatility bars: a candle closes when the sum of the absolute log returns
/// between its trades, `sum |ln(p_i / p_(i-1))|`, reaches `threshold`.
///
/// The sampling then follows the realized price movement, so quiet periods produce few candles
/// and turbulent ones many. In a comparison on Binance BTCUSDT (arXiv 2608.26158), volatility bars
/// built from trades had far less serial dependence than bars built from one-minute data.
///
/// Caveat: trades that alternate between the bid and the ask ("bid-ask bounce") add up
/// without any real price movement, so on a liquid market a tight spread can dominate the sum.
/// A threshold of many spreads keeps that effect small; a [`CusumRule`](crate::CusumRule)
/// ignores such noise altogether.
#[derive(Debug, Clone)]
pub struct VolatilityRule {
    threshold: f64,
    previous_price: Option<f64>,
    sum: f64,
}

impl VolatilityRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold`: The sum of absolute log returns at which a candle closes, e.g. 0.01 for about 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64) -> Result<Self> {
        let mut rule = Self {
            threshold: 1.0,
            previous_price: None,
            sum: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for VolatilityRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        let price = trade.price();
        debug_assert!(price > 0.0, "prices must be positive");
        let Some(previous_price) = self.previous_price.replace(price) else {
            return false;
        };
        self.sum += (price / previous_price).ln().abs();
        debug_assert!(self.sum >= 0.0);
        let trigger = self.sum >= self.threshold;
        if trigger {
            self.sum = 0.0;
        }
        trigger
    }

    /// Restart the sum; the last price stays the reference for the next return.
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl ThresholdRule for VolatilityRule {
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
        CusumRule, Trade,
    };

    fn rule(threshold: f64) -> VolatilityRule {
        VolatilityRule::new(threshold).expect("valid threshold")
    }

    /// Prices that alternate between 100 and `100 * e^step`.
    fn bouncing_prices(n: usize, step: f64) -> Vec<f64> {
        (0..n)
            .map(|i| {
                if i % 2 == 0 {
                    100.0
                } else {
                    100.0 * step.exp()
                }
            })
            .collect()
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(VolatilityRule::new(0.0).is_err());
        assert!(VolatilityRule::new(-0.01).is_err());
        assert!(VolatilityRule::new(f64::NAN).is_err());
        assert!(VolatilityRule::new(f64::INFINITY).is_err());
    }

    #[test]
    fn moves_in_both_directions_add_up() {
        // Each trade moves 0.25%: four moves reach 1%.
        let trades = trades_at_prices(&bouncing_prices(13, 0.0025));
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), vec![4, 8, 12]);
        // The CUSUM filter sees no drift in the same prices.
        let mut cusum = CusumRule::new(0.00999).expect("valid threshold");
        assert!(trigger_indices(&mut cusum, &trades).is_empty());
    }

    #[test]
    fn a_flat_price_never_triggers() {
        let trades = trades_at_prices(&[100.0; 1_000]);
        assert!(trigger_indices(&mut rule(1e-9), &trades).is_empty());
    }

    #[test]
    fn a_single_jump_triggers_at_once() {
        let trades = trades_at_prices(&[100.0, 103.0, 103.0]);
        assert_eq!(trigger_indices(&mut rule(0.01), &trades), vec![1]);
    }

    #[test]
    fn reset_restarts_the_sum_but_keeps_the_reference_price() {
        let mut r = rule(0.00999);
        let trades = trades_at_prices(&bouncing_prices(10, 0.0025));
        assert!(trigger_indices(&mut r, &trades[..3]).is_empty());
        AggregationRule::<OhlcCandle, Trade>::reset(&mut r);
        // The return from trade 2 to 3 counts, so the next four moves end on trade 6.
        assert_eq!(trigger_indices(&mut r, &trades[3..]), vec![3]);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(0.01);
        r.set_threshold(0.02).expect("valid threshold");
        assert_eq!(r.threshold(), 0.02);
        assert!(r.set_threshold(f64::NAN).is_err());
        assert_eq!(r.threshold(), 0.02);
    }
}
