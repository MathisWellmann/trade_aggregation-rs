use crate::{
    price_source::checked_price, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// The prices of the current candle.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Extent {
    open: f64,
    high: f64,
    low: f64,
}

impl Extent {
    fn new(price: f64) -> Self {
        Self {
            open: price,
            high: price,
            low: price,
        }
    }
}

/// Creates range bars: a candle closes when the range of its prices,
/// `(high - low) / open`, would reach `threshold_fraction`.
///
/// Unlike the [`RelativePriceRule`](crate::RelativePriceRule), which measures the distance of the
/// price from the open, this rule measures the distance between the highest and the lowest price,
/// so a candle that swings both ways closes earlier.
/// The event that would reach the range triggers the rule and starts the range of the next candle.
/// With [`TriggerTrade::NextCandle`](crate::TriggerTrade::NextCandle) every closed candle therefore has a range
/// below the threshold, as the range bars of Sierra Chart or NinjaTrader.
///
/// The prices are those of the trades by default. Trades alternate between the bid and the ask
/// ("bid-ask bounce", Roll 1984), so the range of the trade prices is at least the spread
/// even when the market does not move, and a threshold near the spread closes a candle
/// on every bounce. Follow the mid price or the microprice of the quotes instead
/// with [`Self::with_price_source`] to measure the range of the real moves only.
///
/// To calibrate the threshold with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Diffusive`](crate::Scaling::Diffusive): the range of a random walk grows with the
/// square root of the duration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RangeRule<P = TradePrice> {
    threshold_fraction: f64,
    source: P,
    extent: Option<Extent>,
}

impl RangeRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `threshold_fraction`: The range relative to the open at which a candle closes, e.g. 0.01 for 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold_fraction` is not positive and finite.
    pub fn new(threshold_fraction: f64) -> Result<Self> {
        let mut rule = Self {
            threshold_fraction: 1.0,
            source: TradePrice,
            extent: None,
        };
        rule.set_threshold(threshold_fraction)?;
        Ok(rule)
    }
}

impl<P> RangeRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    /// The next price of `source` opens a new range.
    pub fn with_price_source<Q>(self, source: Q) -> RangeRule<Q> {
        RangeRule {
            threshold_fraction: self.threshold_fraction,
            source,
            extent: None,
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for RangeRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(price) = checked_price(&self.source, event) else {
            return false;
        };
        let Some(extent) = self.extent.as_mut() else {
            self.extent = Some(Extent::new(price));
            return false;
        };
        let high = extent.high.max(price);
        let low = extent.low.min(price);
        if (high - low) / extent.open >= self.threshold_fraction {
            *extent = Extent::new(price);
            return true;
        }
        extent.high = high;
        extent.low = low;
        debug_assert!(extent.low <= extent.high);
        debug_assert!((extent.high - extent.low) / extent.open < self.threshold_fraction);
        false
    }

    /// The next price opens a new range.
    fn reset(&mut self) {
        self.extent = None;
    }
}

impl<P> ThresholdRule for RangeRule<P> {
    fn threshold(&self) -> f64 {
        self.threshold_fraction
    }

    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
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
        aggregate_all_trades,
        aggregation_rules::test_utils::{
            quote_event, trade_event, trades_at_prices, trigger_indices,
        },
        load_trades_from_csv,
        plot::OhlcCandle,
        GenericAggregator, Microprice, MidPrice, RelativePriceRule, Trade,
    };

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(RangeRule::new(0.0).is_err());
        assert!(RangeRule::new(-0.01).is_err());
        assert!(RangeRule::new(f64::NAN).is_err());
        assert!(RangeRule::new(f64::INFINITY).is_err());
        assert!(RangeRule::new(0.01).is_ok());
    }

    #[test]
    fn triggers_on_the_range_not_the_distance_from_the_open() {
        let trades = trades_at_prices(&[100.0, 100.6, 99.5, 100.2, 101.2]);
        // Trade 2: 100.6 - 99.5 = 1.1% of 100: trigger, the next range opens at 99.5.
        // Trade 4: 101.2 - 99.5 = 1.7% of 99.5: trigger.
        let mut rule = RangeRule::new(0.01).expect("valid threshold");
        assert_eq!(trigger_indices(&mut rule, &trades), vec![2, 4]);
        // No price before trade 4 is 1% away from the open of 100.
        let mut rule = RelativePriceRule::new(0.01).expect("valid threshold");
        assert_eq!(trigger_indices(&mut rule, &trades), vec![4]);
    }

    #[test]
    fn the_first_trade_opens_the_range() {
        let mut rule = RangeRule::new(0.001).expect("valid threshold");
        assert!(trigger_indices(&mut rule, &trades_at_prices(&[100.0])).is_empty());
        assert_eq!(
            trigger_indices(&mut rule, &trades_at_prices(&[100.2])),
            vec![0]
        );
    }

    #[test]
    fn reset_opens_a_new_range() {
        let mut rule = RangeRule::new(0.01).expect("valid threshold");
        assert!(trigger_indices(&mut rule, &trades_at_prices(&[100.0, 100.9])).is_empty());
        AggregationRule::<Trade>::reset(&mut rule);
        // Measured from the open of 100 this would trigger, from 100.9 it does not.
        assert!(trigger_indices(&mut rule, &trades_at_prices(&[100.95, 101.0])).is_empty());
    }

    #[test]
    fn set_threshold_validates() {
        let mut rule = RangeRule::new(0.01).expect("valid threshold");
        rule.set_threshold(0.02).expect("valid threshold");
        assert_eq!(rule.threshold(), 0.02);
        assert!(rule.set_threshold(0.0).is_err());
        assert_eq!(rule.threshold(), 0.02);
    }

    #[test]
    fn closed_candles_stay_below_the_range() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").expect("test data");
        let threshold = 0.005;
        let mut aggregator = GenericAggregator::<OhlcCandle, RangeRule, Trade>::new(
            RangeRule::new(threshold).expect("valid threshold"),
            false,
        );
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        assert!(candles.len() > 100, "got {} candles", candles.len());
        for c in &candles {
            assert!((c.high() - c.low()) / c.open() < threshold);
            assert!(c.high() >= c.low());
        }
    }

    #[test]
    fn the_mid_price_ignores_the_bid_ask_bounce() {
        let events = [
            quote_event(99.0, 1.0, 101.0, 1.0),
            trade_event(99.0),
            trade_event(101.0),
            quote_event(99.0, 1.0, 101.0, 1.0),
            trade_event(99.0),
            trade_event(101.0),
            quote_event(101.0, 1.0, 103.0, 1.0),
        ];
        // Trades: 99 opens; 101 is 2.02% above it (trigger); 99 is 1.98% below 101 (trigger);
        // 101 is 2.02% above 99 (trigger).
        let mut trades = RangeRule::new(0.015).expect("valid threshold");
        assert_eq!(trigger_indices(&mut trades, &events), vec![2, 4, 5]);
        // Mid: 100 opens, stays, and only the move to 102 (2%) closes the candle.
        let mut mid = RangeRule::new(0.015)
            .expect("valid threshold")
            .with_price_source(MidPrice);
        assert_eq!(trigger_indices(&mut mid, &events), vec![6]);
        assert_eq!(mid.threshold(), 0.015);
    }

    #[test]
    fn the_microprice_follows_the_sizes() {
        // The mid stays at 100, the microprice leans to (99 + 3 * 101) / 4 = 100.5.
        let events = [
            quote_event(99.0, 1.0, 101.0, 1.0),
            trade_event(150.0),
            quote_event(99.0, 3.0, 101.0, 1.0),
        ];
        let rule = RangeRule::new(0.004).expect("valid threshold");
        let mut mid = rule.clone().with_price_source(MidPrice);
        assert!(trigger_indices(&mut mid, &events).is_empty());
        let mut micro = rule.with_price_source(Microprice);
        assert_eq!(trigger_indices(&mut micro, &events), vec![2]);
    }
}
