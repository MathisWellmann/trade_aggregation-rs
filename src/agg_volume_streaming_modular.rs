use crate::common::Trade;
use crate::{By};
use crate::modules::{FeatureModule, FeatureModules, ModularCandle};

#[derive(Debug)]
pub struct AggVolumeStreamingLight {
    pub last_candle: ModularCandle,
    feature_modules: Vec<Box<dyn FeatureModule>>,
    vol_threshold: f64,
    by: By,
    volume: f64,
    init: bool,
}

impl AggVolumeStreamingLight {
    pub fn new(vol_threshold: f64, by: By) -> Self {
        return AggVolumeStreamingLight {
            vol_threshold,
            by,
            last_candle: ModularCandle::default(),
            feature_modules: vec![],
            volume: 0.0,
            init: true,
        }
    }

    pub fn add_feature(&mut self, feature: FeatureModules) {
        let m: Box<dyn FeatureModule> = feature.get_module();
        self.feature_modules.push(m);
    }

    // update observes a trade and updates the aggregated candle
    // return true if new candle has been created
    pub fn update(&mut self, trade: &Trade) -> bool {
        if self.init {
            self.init = false;
            self.volume = 0.0;
            for m in &mut self.feature_modules {
                m.update(trade, true);
            }
        }
        match self.by {
            By::Base => self.volume += trade.size.abs() / trade.price,
            By::Quote => self.volume += trade.size.abs(),
        }
        for m in &mut self.feature_modules {
            m.update(trade, false);
        }

        if self.volume > self.vol_threshold {
            // create new candle
            let c = ModularCandle::from_modules(&self.feature_modules);
            self.last_candle = c;
            self.init = true;
            return true
        }
        return false
    }

    pub fn last(&self) -> &ModularCandle {
        return &self.last_candle
    }

    // sets the volume threshold.
    // caution is adviced as changing it in the middle of candle creation can have unexpected effects
    // it is adviced to only set it after a new candle has been created
    pub fn set_vol_threshold(&mut self, vol_threshold: f64) {
        self.vol_threshold  = vol_threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO:
}
