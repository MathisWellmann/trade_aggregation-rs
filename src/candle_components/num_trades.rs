use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the number of trades
#[derive(Debug, Default, Clone)]
pub struct NumTrades {
    value: usize,
}

impl CandleComponent<usize> for NumTrades {
    #[inline(always)]
    fn value(&self) -> usize {
        self.value
    }

    #[inline(always)]
    fn update(&mut self, _: &Trade) {
        self.value += 1;
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.value = 0;
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
        assert_eq!(m.value(), 10);
    }
}
