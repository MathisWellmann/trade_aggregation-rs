use crate::{AggregationRule, By, Error, Result, TakerTrade, ThresholdRule};

/// Creates order flow ("delta") bars: a candle closes when the net taker volume,
/// buys minus sells, reaches `threshold` in either direction.
///
/// The sign of [`TakerTrade::size`] gives the side of the taker.
/// Balanced buying and selling keeps a candle open, a one-sided flow closes it quickly,
/// so the candles sample when informed, directional trading arrives.
/// The rule is a fixed-threshold version of the volume imbalance bars of
/// López de Prado (Advances in Financial Machine Learning, ch. 2.3.2), whose expected-imbalance
/// threshold is known to collapse or explode; wrap it in [`Adaptive`](crate::Adaptive)
/// for a threshold that follows the activity of the market instead.
#[derive(Debug, Clone)]
pub struct SignedVolumeRule {
    by: By,
    threshold: f64,
    net_volume: f64,
}

impl SignedVolumeRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold`: The absolute net volume at which a candle closes.
    /// `by`: Whether to sum the trade sizes as they are (`By::Quote`) or divided by the price (`By::Base`),
    ///     as for the [`VolumeRule`](crate::VolumeRule).
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64, by: By) -> Result<Self> {
        let mut rule = Self {
            by,
            threshold: 1.0,
            net_volume: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for SignedVolumeRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        debug_assert!(trade.price() > 0.0, "prices must be positive");
        self.net_volume += match self.by {
            By::Quote => trade.size(),
            By::Base => trade.size() / trade.price(),
        };
        let trigger = self.net_volume.abs() >= self.threshold;
        if trigger {
            self.net_volume = 0.0;
        }
        trigger
    }

    fn reset(&mut self) {
        self.net_volume = 0.0;
    }
}

impl ThresholdRule for SignedVolumeRule {
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
        aggregation_rules::test_utils::{trade, trigger_indices},
        plot::OhlcCandle,
        Trade, VolumeRule,
    };

    fn rule(threshold: f64) -> SignedVolumeRule {
        SignedVolumeRule::new(threshold, By::Quote).expect("valid threshold")
    }

    /// Trades one millisecond apart at a price of 100 with the given signed sizes.
    fn trades_with_sizes(sizes: &[f64]) -> Vec<Trade> {
        sizes
            .iter()
            .zip(0..)
            .map(|(size, ts)| trade(ts, 100.0, *size))
            .collect()
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(SignedVolumeRule::new(0.0, By::Quote).is_err());
        assert!(SignedVolumeRule::new(-1.0, By::Base).is_err());
        assert!(SignedVolumeRule::new(f64::NAN, By::Quote).is_err());
        assert!(SignedVolumeRule::new(f64::INFINITY, By::Quote).is_err());
    }

    #[test]
    fn balanced_flow_keeps_the_candle_open() {
        let sizes: Vec<f64> = (0..1_000)
            .map(|i| if i % 2 == 0 { 5.0 } else { -5.0 })
            .collect();
        let trades = trades_with_sizes(&sizes);
        assert!(trigger_indices(&mut rule(10.0), &trades).is_empty());
        // The unsigned volume rule closes a candle every other trade.
        let mut volume = VolumeRule::new(10.0, By::Quote).expect("valid threshold");
        assert_eq!(trigger_indices(&mut volume, &trades).len(), 500);
    }

    #[test]
    fn one_sided_flow_triggers_in_both_directions() {
        let trades = trades_with_sizes(&[4.0, -1.0, 4.0, 3.0, -6.0, -5.0, 2.0]);
        // Net: 4, 3, 7, 10 (trigger), -6, -11 (trigger), 2.
        assert_eq!(trigger_indices(&mut rule(10.0), &trades), vec![3, 5]);
    }

    #[test]
    fn by_base_divides_by_the_price() {
        let trades = [trade(0, 50.0, 100.0), trade(1, 50.0, 100.0)];
        let mut r = SignedVolumeRule::new(4.0, By::Base).expect("valid threshold");
        // 2 + 2 base units.
        assert_eq!(trigger_indices(&mut r, &trades), vec![1]);
    }

    #[test]
    fn reset_clears_the_net_volume() {
        let mut r = rule(10.0);
        let trades = trades_with_sizes(&[8.0, 8.0]);
        assert!(trigger_indices(&mut r, &trades[..1]).is_empty());
        AggregationRule::<OhlcCandle, Trade>::reset(&mut r);
        assert!(trigger_indices(&mut r, &trades[1..]).is_empty());
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(10.0);
        r.set_threshold(20.0).expect("valid threshold");
        assert_eq!(r.threshold(), 20.0);
        assert!(r.set_threshold(0.0).is_err());
        assert_eq!(r.threshold(), 20.0);
    }
}
