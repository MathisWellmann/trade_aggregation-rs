use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Default, Debug)]
pub struct ModuleOpen {
    value: f64,
}

impl FeatureModule for ModuleOpen {
    fn name(&self) -> &str {
        "Open"
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.value = trade.price;
        }
    }
}