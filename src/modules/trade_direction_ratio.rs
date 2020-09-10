use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Debug, Default)]
pub struct ModuleTradeDirectionRatio {
    pub num_buys: usize,
    pub num_trades: usize,
}

impl FeatureModule for ModuleTradeDirectionRatio {
    fn name(&self) -> &str {
        "TradeDirectionRatio"
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