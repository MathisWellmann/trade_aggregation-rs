use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the number of trades
#[derive(Debug, Default, Clone)]
pub struct NumTrades {
    value: f64,
}

impl CandleComponent for NumTrades {
    /// NOTE: this returns f64 out of convenience, but this trait could be made generic in the future
    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, _: &Trade) {
        self.value += 1.0;
    }

    fn reset(&mut self) {
        self.value = 0.0;
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
