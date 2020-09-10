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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_volume() {
        let mut m = ModuleVolume::default();
        let mut sum: f64 = 0.0;
        for t in &crate::modules::tests::TRADES {
            sum += t.size.abs();
            m.update(t, false);
            assert_eq!(m.value(), sum);
        }
    }
}