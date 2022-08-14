use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Debug, Default)]
pub struct ModuleWeightedPrice {
    total_weights: f64,
    weighted_sum: f64,
}

impl FeatureModule for ModuleWeightedPrice {
    fn name(&self) -> &str {
        "WeightedPrice"
    }

    fn value(&self) -> f64 {
        self.weighted_sum / self.total_weights
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.total_weights = 0.0;
            self.weighted_sum = 0.0;
        }
        self.total_weights += trade.size.abs();
        self.weighted_sum += trade.price * trade.size.abs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_weighted_price() {
        let mut m = ModuleWeightedPrice::default();
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
        }
        assert_eq!(m.value(), 102.0);
    }
}
