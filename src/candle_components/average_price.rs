use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the arithmetic mean price.
/// `None` for a candle without trades, instead of dividing 0 by 0.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AveragePrice {
    num_trades: f64,
    price_sum: f64,
}

impl CandleComponent for AveragePrice {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        (self.num_trades > 0.0).then(|| self.price_sum / self.num_trades)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.num_trades = 0.0;
        self.price_sum = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for AveragePrice {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.num_trades += 1.0;
        self.price_sum += trade.price();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_price() {
        let mut m = AveragePrice::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), Some(102.0));
    }

    #[test]
    fn an_empty_candle_has_no_average() {
        let m = AveragePrice::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_is_its_own_average() {
        let mut m = AveragePrice::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 7.0,
            size: 1.0,
        });
        assert_eq!(m.value(), Some(7.0));
    }

    #[test]
    fn reset_forgets_the_trades() {
        let mut m = AveragePrice::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 7.0,
            size: 1.0,
        });
        m.reset();
        assert_eq!(m.value(), None);
    }
}
