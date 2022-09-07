use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the opening price of a Candle
#[derive(Debug, Clone)]
pub struct Open {
    init: bool,
    value: f64,
}

impl Default for Open {
    fn default() -> Self {
        Self {
            init: true,
            value: 0.0,
        }
    }
}

impl CandleComponent for Open {
    /// Returns the open price of the candle
    #[inline(always)]
    fn value(&self) -> f64 {
        self.value
    }
    /// This makes sure the next time "update" is called, the new open value is set
    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for Open {
    /// Only update the open price if this module is in init mode
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            self.value = trade.price();
            self.init = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open() {
        let mut m = Open::default();
        let first_trade = &crate::candle_components::tests::TRADES[0];
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), first_trade.price());
        }
    }
}
