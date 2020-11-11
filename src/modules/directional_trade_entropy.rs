use crate::Trade;
use crate::modules::FeatureModule;

#[derive(Default, Debug)]
pub struct ModuleDirectionalTradeEntropy {
    num_buys: usize,
    num_trades: usize,
}

impl FeatureModule for ModuleDirectionalTradeEntropy {
    fn name(&self) -> &str {
        "directional_trade_entropy"
    }

    fn value(&self) -> f64 {
        let pb: f64 = self.num_buys as f64 / self.num_trades as f64;  // probability of buy direction
        let ps: f64 = 1.0 - pb;  // probability of sell direction
        let mut value: f64 = pb * pb.log2() + ps * ps.log2();
        if value.is_nan() {
            value = 0.0
        }
        -value
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.num_buys = 0;
            self.num_trades = 0;
        }
        if trade.size > 0.0 {
            self.num_buys += 1;
        }
        self.num_trades += 1;
    }
}
