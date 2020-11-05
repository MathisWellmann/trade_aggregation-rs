use crate::Trade;
use crate::modules::FeatureModule;

#[derive(Default, Debug)]
pub struct ModuleAvgSpread {
    bid: f64,
    ask: f64,
    sum: f64,
    count: usize,
    pub value: f64,
}

impl FeatureModule for ModuleAvgSpread {
    fn name(&self) -> &str { "last_spread" }

    fn value(&self) -> f64 { self.value }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.sum = 0.0;
            self.count = 0;
            self.bid = trade.price;
            self.ask = trade.price;
        }
        if trade.size < 0.0 {
            self.bid = trade.price;
        } else {
            self.ask = trade.price;
        }
        self.sum += self.ask - self.bid;
        self.count += 1;
        self.value = self.sum / self.count as f64;
    }
}