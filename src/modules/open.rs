use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Default, Debug)]
pub struct ModuleOpen {
    value: f64,
}

impl FeatureModule for ModuleOpen {
    fn name(&self) -> &str {
        "Open"
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, trade: &Trade, init: bool) {
        if init {
            self.value = trade.price;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_open() {
        let mut m = ModuleOpen::default();
        let first_trade = &crate::modules::tests::TRADES[0];
        m.update(first_trade, true);
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
            assert_eq!(m.value(), first_trade.price);
        }
    }
}
