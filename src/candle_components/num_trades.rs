use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Debug, Default)]
pub struct ModuleNumTrades {
    value: f64,
}

impl FeatureModule for ModuleNumTrades {
    fn name(&self) -> &str {
        "num_trades"
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, _: &Trade, init: bool) {
        if init {
            self.value = 0.0;
        }
        self.value += 1.0;
    }
}
