use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Default, Debug, Clone)]
pub struct ModuleLow {
    pub low: f64,
}

impl FeatureModule for ModuleLow {
    fn name(&self) -> &str {
        "Low"
    }

    fn value(&self) -> f64 {
        self.low
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.low = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_low() {
        let mut m = ModuleLow::default();
        m.update(&crate::modules::tests::TRADES[0], true);
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
        }
        assert_eq!(m.value(), 100.0);
    }
}
