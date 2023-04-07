use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the low price
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Low {
    low: f64,
}

impl Default for Low {
    fn default() -> Self {
        // ensure the initial value is maximal,
        // so any subsequent calls to 'update' will set the proper low value
        Self { low: f64::MAX }
    }
}

impl CandleComponent for Low {
    type Output = f64;
    #[inline(always)]
    fn value(&self) -> f64 {
        self.low
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.low = f64::MAX;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for Low {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if trade.price() < self.low {
            self.low = trade.price();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low() {
        let mut m = Low::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 100.0);
    }
}
