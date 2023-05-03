use std::marker::PhantomData;

use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the number of trades
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NumTrades<T> {
    value: u32,
    phantom: PhantomData<T>,
}

impl CandleComponent<u32> for NumTrades<u32> {
    #[inline(always)]
    fn value(&self) -> u32 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.value = 0;
    }
}
impl<T: TakerTrade> CandleComponentUpdate<T> for NumTrades<u32> {
    #[inline(always)]
    fn update(&mut self, _: &T) {
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
