use crate::{AggregationRule, Error, ModularCandle, Result, TakerTrade};

/// Creates Candles once the price changed by a give relative absolute price delta
#[derive(Debug, Clone)]
pub struct RelativePriceRule {
    init: bool,
    init_price: f64,
    threshold_fraction: f64,
}

impl RelativePriceRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold_fraction`: The relative distance ((`p_t` - `p_i`) / `p_i`) the price needs to move before a new candle creation is triggered.
    ///
    pub fn new(threshold_fraction: f64) -> Result<Self> {
        if threshold_fraction <= 0.0 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            init: true,
            init_price: 0.0,
            threshold_fraction,
        })
    }
}

impl<C, T> AggregationRule<C, T> for RelativePriceRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.init {
            self.init = false;
            self.init_price = trade.price();
            return false;
        }

        let price_delta = (trade.price() - self.init_price).abs() / self.init_price;

        if price_delta >= self.threshold_fraction {
            self.init_price = trade.price();
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_all_trades, load_trades_from_csv,
        plot::{plot_ohlc_candles, OhlcCandle},
        GenericAggregator, Trade,
    };

    #[test]
    fn relative_price_rule() {
        let mut rule = RelativePriceRule::new(0.01).unwrap();

        assert_eq!(
            rule.should_trigger(
                &Trade {
                    timestamp: 0,
                    price: 100.0,
                    size: 10.0
                },
                &OhlcCandle::default()
            ),
            false
        );
        assert_eq!(
            rule.should_trigger(
                &Trade {
                    timestamp: 0,
                    price: 100.5,
                    size: 10.0
                },
                &OhlcCandle::default()
            ),
            false
        );
        assert_eq!(
            rule.should_trigger(
                &Trade {
                    timestamp: 0,
                    price: 101.0,
                    size: 10.0
                },
                &OhlcCandle::default()
            ),
            true
        );
        assert_eq!(
            rule.should_trigger(
                &Trade {
                    timestamp: 0,
                    price: 100.5,
                    size: 10.0
                },
                &OhlcCandle::default()
            ),
            false
        );
        assert_eq!(
            rule.should_trigger(
                &Trade {
                    timestamp: 0,
                    price: 99.0,
                    size: 10.0
                },
                &OhlcCandle::default()
            ),
            true
        );
    }

    #[test]
    fn relative_price_rule_real_data() {
        let trades = load_trades_from_csv("./data/Bitmex_XBTUSD_1M.csv").expect("Unable to load trades at this path, are you sure you're in the root directory of the project?");

        // 0.5% candles
        const THRESHOLD: f64 = 0.005;
        let rule = RelativePriceRule::new(0.01).unwrap();
        let mut aggregator = GenericAggregator::<OhlcCandle, _, Trade>::new(rule);
        let candles = aggregate_all_trades(&trades, &mut aggregator);
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
        let mut aggregator = GenericAggregator::<OhlcCandle, _, Trade>::new(rule);
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        println!("got {} candles", candles.len());

        plot_ohlc_candles(
            &candles,
            "img/relative_price_candles_plot.png",
            (3840, 2160),
        )
        .unwrap();
    }
}
