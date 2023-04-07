use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the volume weighted price
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WeightedPrice {
    total_weights: f64,
    weighted_sum: f64,
}

impl CandleComponent for WeightedPrice {
    type Output = f64;
    #[inline(always)]
    fn value(&self) -> f64 {
        self.weighted_sum / self.total_weights
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.total_weights = 0.0;
        self.weighted_sum = 0.0;
    }
}
impl<T: TakerTrade> CandleComponentUpdate<T> for WeightedPrice {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
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
        assert_eq!(m.value(), 102.0);
    }
}
