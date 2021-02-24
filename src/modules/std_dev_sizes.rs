use crate::modules::FeatureModule;
use crate::Trade;
use crate::welford_online::WelfordOnline;

#[derive(Debug)]
pub struct ModuleStdDevSizes {
    welford: WelfordOnline,
}

impl ModuleStdDevSizes {
    pub fn new() -> Self {
        ModuleStdDevSizes {
            welford: WelfordOnline::new(),
        }
    }
}

impl FeatureModule for ModuleStdDevSizes {
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
        self.welford.add(trade.size);
    }
}