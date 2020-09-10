use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Default, Debug)]
pub struct ModuleHigh {
    pub high: f64,
}

impl FeatureModule for ModuleHigh {
    fn name(&self) -> &str {
        "High"
    }

    fn value(&self) -> f64 {
        self.high
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.high = trade.price;
        }
        if trade.price > self.high {
            self.high = trade.price;
        }
    }
}