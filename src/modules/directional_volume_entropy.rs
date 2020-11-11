use crate::Trade;
use crate::modules::FeatureModule;

#[derive(Default, Debug)]
pub struct ModuleDirectionalVolumeEntropy {
    buy_volume: f64,
    total_volume: f64,
}

impl FeatureModule for ModuleDirectionalVolumeEntropy {
    fn name(&self) -> &str {
        "directional_volume_entropy"
    }

    fn value(&self) -> f64 {
        // sum of absolute volume
        let pb: f64 = self.buy_volume / self.total_volume;  // probability of buy direction
        let ps: f64 = 1.0 - pb;  // probability of sell direction
        let mut value: f64 = pb * pb.log2() + ps * ps.log2();
        if value.is_nan() {
            value = 0.0
        }
        -value
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.buy_volume = 0.0;
            self.total_volume = 0.0;
        }
        self.total_volume += trade.size.abs();
        if trade.size > 0.0 {
            self.buy_volume += trade.size;
        }
    }
}
