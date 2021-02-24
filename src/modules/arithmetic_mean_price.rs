use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Debug, Default)]
pub struct ModuleArithmeticMeanPrice {
    num_trades: f64,
    price_sum: f64,
}

impl FeatureModule for ModuleArithmeticMeanPrice {
    fn name(&self) -> &str {
        "ArithmeticMeanPrice"
    }

    fn value(&self) -> f64 {
        self.price_sum / self.num_trades
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.num_trades = 0.0;
            self.price_sum = 0.0;
        }
        self.num_trades += 1.0;
        self.price_sum += trade.price;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_arithmetic_mean_price() {
        let mut m = ModuleArithmeticMeanPrice::default();
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
        }
        assert_eq!(m.value(), 102.0);
    }
}
