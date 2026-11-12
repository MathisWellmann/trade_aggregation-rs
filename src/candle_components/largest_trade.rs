use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// The size of the largest trade in the base currency, 0 for a candle without trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LargestTrade {
    largest: f64,
}

impl CandleComponent for LargestTrade {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.largest
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.largest = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for LargestTrade {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.largest = self.largest.max(trade.base_size().abs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largest_trade_is_the_largest_absolute_size() {
        let mut m = LargestTrade::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        // 30 at a price of 102 is the largest base size.
        assert_eq!(m.value(), 30.0 / 102.0);
        m.reset();
        assert_eq!(m.value(), 0.0);
    }
}
