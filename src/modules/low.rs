use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Default, Debug, Clone)]
pub struct ModuleLow {
    pub low: f64,
}

impl FeatureModule for ModuleLow {
    fn name(&self) -> &str {
        "Low"
    }

    fn value(&self) -> f64 {
        self.low
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.low = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
    }
}