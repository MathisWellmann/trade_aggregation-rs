use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the volume weighted price
#[derive(Debug, Default, Clone)]
pub struct WeightedPrice {
    total_weights: f64,
    weighted_sum: f64,
}

impl CandleComponent for WeightedPrice {
    fn value(&self) -> f64 {
        self.weighted_sum / self.total_weights
    }

    fn update(&mut self, trade: &Trade) {
        self.total_weights += trade.size.abs();
        self.weighted_sum += trade.price * trade.size.abs();
    }

    fn reset(&mut self) {
        self.total_weights = 0.0;
        self.weighted_sum = 0.0;
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
        assert_eq!(m.value(), 102.0);
    }
}
