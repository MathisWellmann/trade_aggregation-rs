use crate::modules::FeatureModule;
use crate::Trade;

/// Measures the velocity of candle creation based on the formula:
/// 1.0 / t  , where t is measured in minutes
/// The higher the velocity the faster the candle has been created
#[derive(Debug, Default)]
pub struct ModuleTimeVelocity {
    init_time: u64,
    last_time: u64,
}

impl FeatureModule for ModuleTimeVelocity {
    fn name(&self) -> &str {
        "time_velocity"
    }

    fn value(&self) -> f64 {
        // elapsed time in minutes, assumes timestamp are in milliseconds
        let elapsed_m: f64 = (self.last_time - self.init_time) as f64 / 60_000.0;
        1.0 / elapsed_m
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.init_time = trade.timestamp;
        }
        self.last_time = trade.timestamp;
    }
}

