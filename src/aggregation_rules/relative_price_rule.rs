use crate::{AggregationRule, Error, ModularCandle, Result, TakerTrade};

/// Creates Candles once the price changed by a give relative absolute price delta
pub struct RelativePriceRule {
    init: bool,
    init_price: f64,
    threshold_delta: f64,
}

impl RelativePriceRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold_delta`: The trigger condition
    ///
    pub fn new(threshold_delta: f64) -> Result<Self> {
        if threshold_delta <= 0.0 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            init: true,
            init_price: 0.0,
            threshold_delta,
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

        if price_delta >= self.threshold_delta {
            self.init = true;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::{plot::OhlcCandle, Trade};

    use super::*;

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
}
