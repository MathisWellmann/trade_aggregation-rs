use crate::{aggregator::CandleComponent, Trade};

/// The Candle component "Open" keeps track of the opening price
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
    fn value(&self) -> f64 {
        self.value
    }

    /// Only update the open price if this module is in init mode
    fn update(&mut self, trade: &Trade) {
        if self.init {
            self.value = trade.price;
            self.init = false;
        }
    }

    /// This makes sure the next time "update" is called, the new open value is set
    fn reset(&mut self) {
        self.init = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_open() {
        let mut m = Open::default();
        let first_trade = &crate::candle_components::tests::TRADES[0];
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), first_trade.price);
        }
    }
}
