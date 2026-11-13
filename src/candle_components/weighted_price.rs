use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the volume weighted price.
/// `None` for a candle without trades, or if every trade had a zero size,
/// instead of dividing 0 by 0.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeightedPrice {
    total_weights: f64,
    weighted_sum: f64,
}

impl CandleComponent for WeightedPrice {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        (self.total_weights > 0.0).then(|| self.weighted_sum / self.total_weights)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.total_weights = 0.0;
        self.weighted_sum = 0.0;
    }
}
impl<E: AsTrade> CandleComponentUpdate<E> for WeightedPrice {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.total_weights += trade.size().abs();
        self.weighted_sum += trade.price() * trade.size().abs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_price() {
        let mut m = WeightedPrice::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), Some(102.0));
    }

    #[test]
    fn an_empty_candle_has_no_weighted_price() {
        let m = WeightedPrice::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn trades_of_zero_size_carry_no_weight() {
        let mut m = WeightedPrice::default();
        for size in [0.0, -0.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 123.0,
                size,
            });
        }
        assert_eq!(m.value(), None, "zero size everywhere is still 0 / 0");
    }

    #[test]
    fn reset_forgets_the_trades() {
        let mut m = WeightedPrice::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 7.0,
            size: 2.0,
        });
        m.reset();
        assert_eq!(m.value(), None);
    }
}
