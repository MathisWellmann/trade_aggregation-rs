use crate::modules::{FeatureModule, FeatureModules, ModularCandle};
use crate::{By, Trade};

#[derive(Debug)]
/// The modular equivalent of VolumeAggregator
pub struct ModularVolumeAggregator {
    feature_modules: Vec<Box<dyn FeatureModule>>,
    vol_threshold: f64,
    by: By,
    volume: f64,
    init: bool,
}

impl ModularVolumeAggregator {
    /// Create a new modular volume aggregator
    ///
    /// # Parameters:
    /// - vol_threshold: create a new candle after this total volume has been reached
    /// - by: determines how to interpret the trade size, either as denoted in QUOTE or in BASE
    pub fn new(vol_threshold: f64, by: By) -> Self {
        ModularVolumeAggregator {
            vol_threshold,
            by,
            feature_modules: vec![],
            volume: 0.0,
            init: true,
        }
    }

    /// Add a feature module to gain a new candle feature
    pub fn add_feature(&mut self, feature: FeatureModules) {
        let m: Box<dyn FeatureModule> = feature.get_module();
        self.feature_modules.push(m);
    }

    /// Adds a new trade to aggregation
    /// Returns Some(Candle) only when a new candle has been created,
    /// otherwise it returns None
    pub fn update(&mut self, trade: &Trade) -> Option<ModularCandle> {
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
            self.init = true;
            return Some(c);
        }
        None
    }
}
