use crate::{CandleComponent, Trade};

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
    fn update(&mut self, trade: &Trade) {
        self.value = trade.price
    }

    #[inline(always)]
    fn reset(&mut self) {}
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
