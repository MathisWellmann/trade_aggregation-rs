use crate::Trade;
use crate::modules::FeatureModule;

#[derive(Default, Debug)]
pub struct ModuleLastSpread {
    bid: f64,
    ask: f64,
    pub value: f64,
}

impl FeatureModule for ModuleLastSpread {
    fn name(&self) -> &str { "last_spread" }

    fn value(&self) -> f64 { self.value }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.bid = trade.price;
            self.ask = trade.price;
        }
        if trade.size < 0.0 {
            self.bid = trade.price;
        } else {
            self.ask = trade.price;
        }
        self.value = self.ask - self.bid;
    }
}