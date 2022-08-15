use crate::{CandleComponent, Trade};

#[derive(Default, Debug, Clone)]
pub struct Close {
    pub value: f64,
}

impl CandleComponent for Close {
    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, trade: &Trade) {
        self.value = trade.price
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_close() {
        let mut m = Close::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), t.price);
        }
        assert_eq!(m.value(), crate::candle_components::tests::TRADES[9].price);
    }
}
