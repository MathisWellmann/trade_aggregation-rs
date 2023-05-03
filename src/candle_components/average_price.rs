use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the arithmetic mean price
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AveragePrice {
    num_trades: f64,
    price_sum: f64,
}

impl CandleComponent<f64> for AveragePrice {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.price_sum / self.num_trades
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.num_trades = 0.0;
        self.price_sum = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for AveragePrice {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
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
        assert_eq!(m.value(), 102.0);
    }
}
