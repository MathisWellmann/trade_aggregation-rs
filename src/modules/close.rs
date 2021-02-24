use crate::modules::FeatureModule;
use crate::Trade;

#[derive(Default, Debug)]
pub struct ModuleClose {
    pub value: f64,
}

impl FeatureModule for ModuleClose {
    fn name(&self) -> &str {
        "Close"
    }

    fn value(&self) -> f64 {
        self.value
    }

    fn update(&mut self, trade: &Trade, _: bool) {
        self.value = trade.price
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_close() {
        let mut m = ModuleClose::default();
        for t in &crate::modules::tests::TRADES {
            m.update(t, false);
            assert_eq!(m.value(), t.price);
        }
        assert_eq!(m.value(), crate::modules::tests::TRADES[9].price);
    }
}