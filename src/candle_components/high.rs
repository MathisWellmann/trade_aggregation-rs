use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the high price
#[derive(Default, Debug, Clone)]
pub struct High {
    high: f64,
}

impl CandleComponent for High {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.high
    }

    #[inline(always)]
    fn update(&mut self, trade: &Trade) {
        if trade.price > self.high {
            self.high = trade.price;
        }
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.high = std::f64::MIN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high() {
        let mut m = High::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 105.0);
    }
}
