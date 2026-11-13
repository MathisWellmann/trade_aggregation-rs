use crate::{
    price_source::LogReturns, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// Creates realized variance bars on a "business time" clock: a candle closes when the sum of
/// the squared log returns between its prices, `sum ln(p_i / p_(i-1))^2`, reaches `threshold`.
///
/// By the theorem of Dambis, Dubins and Schwarz, a continuous martingale is a Brownian motion
/// run on the clock of its quadratic variation. Sampled at equal increments of that clock,
/// the price has independent Gaussian increments (Clark 1973; Ané and Geman 2000,
/// "Order flow, transaction clock, and normality of asset returns"),
/// so the log returns of these candles are close to `N(0, threshold)` in any volatility regime:
/// homoskedastic, normal inputs for a downstream model, and comparable across symbols.
/// The candle closes on the return that crosses the threshold, so its realized variance
/// overshoots `threshold` by at most that last squared return.
///
/// Unlike the [`VolatilityRule`](crate::VolatilityRule), which sums absolute returns,
/// a large return counts quadratically: one return of 1% adds as much as 100 returns of 0.1%,
/// so a jump closes the candle at once.
///
/// The prices are those of the trades by default. Trade prices bounce between the bid and the
/// ask (Roll 1984), and every trade return adds the variance of this microstructure noise,
/// so the realized variance of trade prices grows with the number of trades rather than with
/// the moves of the market (Zhang, Mykland and Aït-Sahalia 2005). Prefer the mid price or the
/// microprice of the quotes with [`Self::with_price_source`].
///
/// To calibrate the threshold with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Linear`](crate::Scaling::Linear): the quadratic variation grows in proportion
/// to the duration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VarianceRule<P = TradePrice> {
    threshold: f64,
    returns: LogReturns<P>,
    sum: f64,
}

impl VarianceRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `threshold`: The sum of squared log returns at which a candle closes,
    ///     e.g. 1e-4 for candles whose returns have a standard deviation of about 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64) -> Result<Self> {
        let mut rule = Self {
            threshold: 1.0,
            returns: LogReturns::new(TradePrice),
            sum: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

impl<P> VarianceRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    pub fn with_price_source<Q>(self, source: Q) -> VarianceRule<Q> {
        VarianceRule {
            threshold: self.threshold,
            returns: LogReturns::new(source),
            sum: 0.0,
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for VarianceRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return false;
        };
        debug_assert!(r.is_finite(), "the log return of checked prices is finite");
        self.sum += r * r;
        debug_assert!(self.sum >= 0.0);
        let trigger = self.sum >= self.threshold;
        if trigger {
            self.sum = 0.0;
        }
        debug_assert!(self.sum < self.threshold);
        trigger
    }

    /// Restart the sum; the last price stays the reference for the next return,
    /// so no return is lost between candles.
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl<P> ThresholdRule for VarianceRule<P> {
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
        aggregation_rules::test_utils::{
            prices_from_returns, quote_event, trade_event, trades_at_prices, trigger_indices,
        },
        MidPrice, PriceKind, Trade, VolatilityRule,
    };

    fn rule(threshold: f64) -> VarianceRule {
        VarianceRule::new(threshold).expect("valid threshold")
    }

    /// Log returns of `size` that alternate in sign, starting upwards.
    fn alternating(n: usize, size: f64) -> Vec<f64> {
        (0..n)
            .map(|i| if i % 2 == 0 { size } else { -size })
            .collect()
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(VarianceRule::new(0.0).is_err());
        assert!(VarianceRule::new(-1e-4).is_err());
        assert!(VarianceRule::new(f64::NAN).is_err());
        assert!(VarianceRule::new(f64::INFINITY).is_err());
        assert!(VarianceRule::new(1e-4).is_ok());
    }

    #[test]
    fn squared_returns_in_both_directions_add_up() {
        // Each return adds 1e-4: the third return of a candle reaches 2.99e-4.
        let trades = trades_at_prices(&prices_from_returns(&alternating(10, 0.01)));
        assert_eq!(trigger_indices(&mut rule(2.99e-4), &trades), vec![3, 6, 9]);
    }

    #[test]
    fn a_jump_counts_quadratically() {
        // 50 returns of 0.1% add 5e-5, the jump of 1% adds 1e-4 at once.
        let mut returns = vec![0.001; 50];
        returns.push(0.01);
        let trades = trades_at_prices(&prices_from_returns(&returns));
        assert_eq!(trigger_indices(&mut rule(1e-4), &trades), vec![51]);
        // The sum of absolute returns closes a candle every ten small returns as well.
        let mut volatility = VolatilityRule::new(0.0099).expect("valid threshold");
        assert_eq!(
            trigger_indices(&mut volatility, &trades),
            vec![10, 20, 30, 40, 50, 51]
        );
    }

    #[test]
    fn a_flat_price_never_triggers() {
        let trades = trades_at_prices(&[100.0; 1_000]);
        assert!(trigger_indices(&mut rule(1e-12), &trades).is_empty());
    }

    #[test]
    fn the_first_price_has_no_return() {
        let mut r = rule(1e-4);
        assert!(trigger_indices(&mut r, &trades_at_prices(&[200.0])).is_empty());
        assert_eq!(
            trigger_indices(&mut r, &trades_at_prices(&[100.0])),
            vec![0]
        );
    }

    #[test]
    fn reset_restarts_the_sum_but_keeps_the_reference_price() {
        let mut r = rule(2.99e-4);
        let trades = trades_at_prices(&prices_from_returns(&alternating(10, 0.01)));
        assert!(trigger_indices(&mut r, &trades[..3]).is_empty());
        AggregationRule::<Trade>::reset(&mut r);
        // The return from trade 2 to 3 counts, so the third return ends on trade 5.
        assert_eq!(trigger_indices(&mut r, &trades[3..]), vec![2, 5]);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(1e-4);
        r.set_threshold(2e-4).expect("valid threshold");
        assert_eq!(r.threshold(), 2e-4);
        assert!(r.set_threshold(0.0).is_err());
        assert_eq!(r.threshold(), 2e-4);
    }

    #[test]
    fn the_mid_price_ignores_the_bid_ask_bounce() {
        let events: Vec<_> = (0..4)
            .flat_map(|_| {
                [
                    quote_event(99.0, 1.0, 101.0, 1.0),
                    trade_event(99.0),
                    trade_event(101.0),
                ]
            })
            .collect();
        // Each bounce adds ln(101 / 99)^2 = 4.0e-4: trades 2, 4 and 5 reach 1e-3,
        // then trades 7, 8 and 10.
        assert_eq!(trigger_indices(&mut rule(1e-3), &events), vec![5, 10]);
        for mut mid in [
            rule(1e-3).with_price_source(PriceKind::Mid),
            rule(1e-3).with_price_source(PriceKind::Microprice),
        ] {
            assert!(trigger_indices(&mut mid, &events).is_empty());
        }
        // A move of the mid by 2% adds ln(1.02)^2 = 3.92e-4.
        let mut mid = rule(3.9e-4).with_price_source(MidPrice);
        let mut moved = events.clone();
        moved.push(quote_event(101.0, 1.0, 103.0, 1.0));
        assert_eq!(trigger_indices(&mut mid, &moved), vec![12]);
    }
}
