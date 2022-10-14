use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the number of trades
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NumTrades {
    value: f64,
}

impl CandleComponent for NumTrades {
    /// NOTE: this returns f64 out of convenience, but this trait could be made generic in the future
    #[inline(always)]
    fn value(&self) -> f64 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.value = 0.0;
    }
}
impl<T: TakerTrade> CandleComponentUpdate<T> for NumTrades {
    /// NOTE: this returns f64 out of convenience, but this trait could be made generic in the future

    #[inline(always)]
    fn update(&mut self, _: &T) {
        self.value += 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_trades() {
        let mut m = NumTrades::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 10.0);
    }
}
