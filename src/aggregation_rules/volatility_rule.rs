use crate::{
    price_source::LogReturns, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// Creates volatility bars: a candle closes when the sum of the absolute log returns
/// between its prices, `sum |ln(p_i / p_(i-1))|`, reaches `threshold`.
///
/// The sampling then follows the realized price movement, so quiet periods produce few candles
/// and turbulent ones many. In a comparison on Binance BTCUSDT (arXiv 2608.26158), volatility bars
/// built from trades had far less serial dependence than bars built from one-minute data.
///
/// The prices are those of the trades by default. Trades that alternate between the bid and
/// the ask ("bid-ask bounce") add up without any real price movement, so on a liquid market
/// a tight spread can dominate the sum. Follow the mid price or the microprice of the quotes
/// instead with [`Self::with_price_source`], or use a threshold of many spreads;
/// a [`CusumRule`](crate::CusumRule) ignores such noise altogether.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolatilityRule<P = TradePrice> {
    threshold: f64,
    returns: LogReturns<P>,
    sum: f64,
}

impl VolatilityRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `threshold`: The sum of absolute log returns at which a candle closes, e.g. 0.01 for about 1%.
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

impl<P> VolatilityRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    pub fn with_price_source<Q>(self, source: Q) -> VolatilityRule<Q> {
        VolatilityRule {
            threshold: self.threshold,
            returns: LogReturns::new(source),
            sum: 0.0,
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for VolatilityRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return false;
        };
        self.sum += r.abs();
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

impl<P> ThresholdRule for VolatilityRule<P> {
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
        AggregationRule::<Trade>::reset(&mut r);
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

    #[test]
    fn the_mid_price_ignores_the_bid_ask_bounce() {
        use crate::{MarketEvent, MidPrice, Quote};
        let quote = MarketEvent::Quote(Quote {
            timestamp: 0,
            bid_price: 99.0,
            bid_size: 1.0,
            ask_price: 101.0,
            ask_size: 1.0,
        });
        let at = |price: f64| {
            MarketEvent::Trade(Trade {
                timestamp: 0,
                price,
                size: 1.0,
            })
        };
        let events: Vec<_> = (0..100)
            .flat_map(|_| [quote, at(99.0), at(101.0)])
            .collect();
        let mut trades = rule(0.05);
        let mut mid = rule(0.05).with_price_source(MidPrice);
        let fired = |r: &mut dyn AggregationRule<MarketEvent<Trade, Quote>>| {
            events.iter().filter(|e| r.should_trigger(e)).count()
        };
        assert!(fired(&mut trades) > 0);
        assert_eq!(fired(&mut mid), 0);
        assert_eq!(mid.threshold(), 0.05);
    }
}
