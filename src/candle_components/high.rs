use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the high price
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct High {
    high: f64,
}

impl CandleComponent<f64> for High {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.high
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.high = std::f64::MIN;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for High {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if trade.price() > self.high {
            self.high = trade.price();
        }
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
