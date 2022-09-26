use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// Computes the median price from a sorted list of trade prices
#[derive(Debug, Default, Clone)]
pub struct MedianPrice {
    prices: Vec<f64>,
}

impl CandleComponent for MedianPrice {
    #[inline(always)]
    fn value(&self) -> f64 {
        let mut prices = self.prices.clone();
        prices.sort_by(|a, b| a.partial_cmp(b).unwrap());

        prices[prices.len() / 2]
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.prices.clear();
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for MedianPrice {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.prices.push(trade.price());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_price() {
        let mut m = MedianPrice::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 102.0);
    }
}
