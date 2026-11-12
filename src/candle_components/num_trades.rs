use crate::{AsTrade, CandleComponent, CandleComponentUpdate};

/// This 'CandleComponent' keeps track of the number of trades
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NumTrades<T> {
    value: T,
}

impl CandleComponent for NumTrades<u32> {
    type Value = u32;

    #[inline(always)]
    fn value(&self) -> u32 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.value = 0;
    }
}
impl<E: AsTrade> CandleComponentUpdate<E> for NumTrades<u32> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        if event.as_trade().is_none() {
            return;
        }
        self.value += 1;
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
