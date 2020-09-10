use crate::modules::FeatureModule;
use crate::common::Trade;

#[derive(Debug, Default)]
pub struct ModuleVolumeDirectionRatio {
    volume: f64,
    buy_volume: f64,
}

impl FeatureModule for ModuleVolumeDirectionRatio {
    fn name(&self) -> &str {
        "VolumeDirectionRatio"
    }

    fn value(&self) -> f64 {
        self.buy_volume / self.volume
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.volume = 0.0;
            self.buy_volume = 0.0;
        }
        self.volume += trade.size.abs();
        if trade.size > 0.0 {
            self.buy_volume += trade.size.abs();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use round::round;

    #[test]
    fn module_volume_direction_ratio() {
        let mut m = ModuleVolumeDirectionRatio::default();
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
        }
        assert_eq!(round(m.value(), 4), 0.7143);
    }
}