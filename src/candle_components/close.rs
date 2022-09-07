use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the close price
#[derive(Default, Debug, Clone)]
pub struct Close {
    value: f64,
}

impl CandleComponent for Close {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

impl<T: TakerTrade> CandleComponentUpdate<T> for Close {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.value = trade.price()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close() {
        let mut m = Close::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), t.price);
        }
        assert_eq!(m.value(), crate::candle_components::tests::TRADES[9].price);
    }
}
