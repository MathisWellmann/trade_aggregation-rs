use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Debug, Default)]
pub struct ModuleDirectionalTradeRatio {
    pub num_buys: usize,
    pub num_trades: usize,
}

impl FeatureModule for ModuleDirectionalTradeRatio {
    fn name(&self) -> &str {
        "directional_trade_ratio"
    }

    fn value(&self) -> f64 {
        self.num_buys as f64 / self.num_trades as f64
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.num_buys = 0;

            self.num_trades = 0;
        }
        self.num_trades += 1;
        if trade.size > 0.0 {
            self.num_buys += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_trade_direction_ratio() {
        let mut m = ModuleDirectionalTradeRatio::default();
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
        }
        assert_eq!(m.value(), 0.7);
    }
}
