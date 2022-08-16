use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the arithmetic mean price
#[derive(Debug, Default, Clone)]
pub struct AveragePrice {
    num_trades: f64,
    price_sum: f64,
}

impl CandleComponent for AveragePrice {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.price_sum / self.num_trades
    }

    #[inline(always)]
    fn update(&mut self, trade: &Trade) {
        self.num_trades += 1.0;
        self.price_sum += trade.price;
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.num_trades = 0.0;
        self.price_sum = 0.0;
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
