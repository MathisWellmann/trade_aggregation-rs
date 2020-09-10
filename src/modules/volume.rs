use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Debug, Default)]
pub struct ModuleVolume {
    volume: f64,
}

impl FeatureModule for ModuleVolume {
    fn name(&self) -> &str {
        "Volume"
    }

    fn value(&self) -> f64 {
        self.volume
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.volume = 0.0;
        }
        self.volume += trade.size.abs()
    }
}