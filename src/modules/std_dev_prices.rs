use crate::modules::FeatureModule;
use crate::common::Trade;
use crate::welford_online::WelfordOnline;

#[derive(Debug)]
pub struct ModuleStdDevPrices {
    welford: WelfordOnline,
}

impl ModuleStdDevPrices {
    pub fn new() -> Self {
        ModuleStdDevPrices {
            welford: WelfordOnline::new(),
        }
    }
}

impl FeatureModule for ModuleStdDevPrices {
    fn name(&self) -> &str {
        "StdDevPrices"
    }

    fn value(&self) -> f64 {
        self.welford.std_dev()
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.welford = WelfordOnline::new();
        }
        self.welford.add(trade.price);
    }
}