use crate::{
    price_source::checked_price, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// Creates candles once the price moved `threshold_fraction` away from a reference price,
/// in either direction (similar to Renko bricks).
///
/// The first price is the reference; the price that triggers becomes the next reference.
///
/// The prices are those of the trades by default. Trades alternate between the bid and the ask
/// ("bid-ask bounce", Roll 1984), so a threshold near the spread closes a candle on every bounce
/// even when the market does not move. Follow the mid price or the microprice of the quotes
/// instead with [`Self::with_price_source`] to sample the real moves only.
///
/// To calibrate the threshold with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Diffusive`](crate::Scaling::Diffusive): the distance a random walk travels grows
/// with the square root of the duration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelativePriceRule<P = TradePrice> {
    threshold_fraction: f64,
    source: P,
    /// The price the next move is measured from, `None` until the first price.
    reference: Option<f64>,
}

impl RelativePriceRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `threshold_fraction`: The relative distance (`|p_t - p_ref| / p_ref`) the price needs to move
    ///     before a new candle creation is triggered, e.g. 0.01 for 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold_fraction` is not positive and finite.
    pub fn new(threshold_fraction: f64) -> Result<Self> {
        let mut rule = Self {
            threshold_fraction: 1.0,
            source: TradePrice,
            reference: None,
        };
        rule.set_threshold(threshold_fraction)?;
        Ok(rule)
    }
}

impl<P> RelativePriceRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    /// The next price of `source` becomes the reference.
    pub fn with_price_source<Q>(self, source: Q) -> RelativePriceRule<Q> {
        RelativePriceRule {
            threshold_fraction: self.threshold_fraction,
            source,
            reference: None,
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for RelativePriceRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(price) = checked_price(&self.source, event) else {
            return false;
        };
        let Some(reference) = self.reference else {
            self.reference = Some(price);
            return false;
        };
        debug_assert!(reference > 0.0, "the reference is a checked price");
        let price_delta = (price - reference).abs() / reference;
        if price_delta >= self.threshold_fraction {
            self.reference = Some(price);
            return true;
        }
        false
    }

    /// The next price becomes the reference price.
    fn reset(&mut self) {
        self.reference = None;
    }
}

impl<P> ThresholdRule for RelativePriceRule<P> {
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
        plot::{plot_ohlc_candles, OhlcCandle},
        GenericAggregator, MidPrice, Trade,
    };

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(RelativePriceRule::new(0.0).is_err());
        assert!(RelativePriceRule::new(-0.01).is_err());
        assert!(RelativePriceRule::new(f64::NAN).is_err());
        assert!(RelativePriceRule::new(f64::INFINITY).is_err());
        assert!(RelativePriceRule::new(0.01).is_ok());
    }

    #[test]
    fn reset_makes_the_next_price_the_reference() {
        let mut rule = RelativePriceRule::new(0.01).expect("valid threshold");
        assert!(trigger_indices(&mut rule, &trades_at_prices(&[100.0, 100.9])).is_empty());
        AggregationRule::<Trade>::reset(&mut rule);
        // 101.5 is 1.5% above the first reference of 100, but only 0.54% above 100.95.
        assert!(trigger_indices(&mut rule, &trades_at_prices(&[100.95, 101.5])).is_empty());
        assert_eq!(
            trigger_indices(&mut rule, &trades_at_prices(&[102.0])),
            vec![0]
        );
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
        // Trades: 99 is the reference; 101 is 2.02% above it (trigger, new reference);
        // 99 is 1.98% below 101 (trigger); 101 is 2.02% above 99 (trigger).
        let mut trades = RelativePriceRule::new(0.015).expect("valid threshold");
        assert_eq!(trigger_indices(&mut trades, &events), vec![2, 4, 5]);
        // Mid: 100 is the reference and only the move to 102 (2%) closes the candle.
        let mut mid = RelativePriceRule::new(0.015)
            .expect("valid threshold")
            .with_price_source(MidPrice);
        assert_eq!(trigger_indices(&mut mid, &events), vec![6]);
        assert_eq!(mid.threshold(), 0.015);
    }
    #[test]
    fn relative_price_rule() {
        let mut rule = RelativePriceRule::new(0.01).unwrap();

        assert!(!rule.should_trigger(&Trade {
            timestamp: 0,
            price: 100.0,
            size: 10.0
        }));
        assert!(!rule.should_trigger(&Trade {
            timestamp: 0,
            price: 100.5,
            size: 10.0
        }));
        assert!(rule.should_trigger(&Trade {
            timestamp: 0,
            price: 101.0,
            size: 10.0
        }));
        assert!(!rule.should_trigger(&Trade {
            timestamp: 0,
            price: 100.5,
            size: 10.0
        }));
        assert!(rule.should_trigger(&Trade {
            timestamp: 0,
            price: 99.0,
            size: 10.0
        }));
    }

    #[test]
    fn relative_price_rule_real_data() {
        let trades = load_trades_from_csv("./data/Bitmex_XBTUSD_1M.csv").expect("Unable to load trades at this path, are you sure you're in the root directory of the project?");

        // 0.5% candles
        const THRESHOLD: f64 = 0.005;
        let rule = RelativePriceRule::new(0.01).unwrap();
        let mut aggregator = GenericAggregator::<OhlcCandle, _, Trade>::new(rule, false);
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        assert!(!candles.is_empty());

        for c in candles {
            assert!((c.high() - c.low()) / c.low() >= THRESHOLD);
            assert!((c.close() - c.open()).abs() / c.open() >= THRESHOLD);
        }
    }

    #[test]
    fn relative_price_candles_plot() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        const THRESHOLD: f64 = 0.005;
        let rule = RelativePriceRule::new(THRESHOLD).unwrap();
        let mut aggregator = GenericAggregator::<OhlcCandle, _, Trade>::new(rule, false);
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        println!("got {} candles", candles.len());

        plot_ohlc_candles(
            &candles,
            "img/relative_price_candles_plot.png",
            (3840, 2160),
        )
        .unwrap();
    }
}
