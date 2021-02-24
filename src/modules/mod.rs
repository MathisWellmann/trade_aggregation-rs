use crate::Trade;
use crate::modules::close::ModuleClose;
use crate::modules::open::ModuleOpen;
use crate::modules::high::ModuleHigh;
use crate::modules::low::ModuleLow;
use std::fmt::{Debug};
use crate::modules::weighted_price::ModuleWeightedPrice;
use crate::modules::directional_trade_ratio::ModuleDirectionalTradeRatio;
use crate::modules::directional_volume_ratio::ModuleDirectionalVolumeRatio;
use crate::modules::std_dev_prices::ModuleStdDevPrices;
use crate::modules::std_dev_sizes::ModuleStdDevSizes;
use crate::modules::volume::ModuleVolume;
use crate::modules::arithmetic_mean_price::ModuleArithmeticMeanPrice;
use crate::modules::time_velocity::ModuleTimeVelocity;
use crate::modules::num_trades::ModuleNumTrades;

mod open;
mod high;
mod low;
mod close;
mod volume;
mod directional_trade_ratio;
mod directional_volume_ratio;
mod std_dev_sizes;
mod std_dev_prices;
mod weighted_price;
mod arithmetic_mean_price;
mod time_velocity;
mod num_trades;

#[derive(Debug, Default)]
/// Holds candle features in a vector
pub struct ModularCandle {
    features: Vec<f64>,
}

impl ModularCandle {
    /// Create a new ModularCandle from feature modules
    pub fn from_modules(modules: &Vec<Box<dyn FeatureModule>>) -> Self {
        let mut features: Vec<f64> = Vec::new();
        for m in modules {
            features.push(m.value());
        }
        Self {
            features,
        }
    }

    /// Return a reference to the features of the modular candle
    pub fn get_features(&self) -> &Vec<f64> {
        &self.features
    }
}

/// enumeration of all available features
pub enum FeatureModules {
    // TODO: how to integrate different types for FeatureModule
    // Timestamp,
    /// The open price of a candle
    Open,
    /// The high price of a candle
    High,
    /// The low price of a candle
    Low,
    /// The close price of a candle
    Close,
    /// The sum of trade sizes of a candle
    Volume,
    /// Equally weighted price of a candle
    ArithmeticMeanPrice,
    /// Volume weighted price of a candle
    WeightedPrice,
    /// Number of trades that happened during that candle
    NumTrades,
    /// #buys / #trades
    DirectionalTradeRatio,
    /// buy_volume / total_volume
    DirectionalVolumeRatio,
    /// Standard deviation of prices from trades that happened during the candle
    StdDevPrices,
    /// Standard deviation of sizes from trades that happened during the candle
    StdDevSizes,
    /// Measures the speed of candle creation: 1.0 / elapsed_m
    TimeVelocity,
}

impl FeatureModules {
    /// Return the associated boxed Struct for a module
    pub fn get_module(&self) -> Box<dyn FeatureModule> {
        return match self {
            // FeatureModules::Timestamp => Box::new(ModuleTimestamp::default()),
            FeatureModules::Open => Box::new(ModuleOpen::default()),
            FeatureModules::High => Box::new(ModuleHigh::default()),
            FeatureModules::Low => Box::new(ModuleLow::default()),
            FeatureModules::Close => Box::new(ModuleClose::default()),
            FeatureModules::Volume => Box::new(ModuleVolume::default()),
            FeatureModules::ArithmeticMeanPrice => Box::new(ModuleArithmeticMeanPrice::default()),
            FeatureModules::WeightedPrice => Box::new(ModuleWeightedPrice::default()),
            FeatureModules::NumTrades => Box::new(ModuleNumTrades::default()),
            FeatureModules::DirectionalTradeRatio => Box::new(ModuleDirectionalTradeRatio::default()),
            FeatureModules::DirectionalVolumeRatio => Box::new(ModuleDirectionalVolumeRatio::default()),
            FeatureModules::StdDevPrices => Box::new(ModuleStdDevPrices::new()),
            FeatureModules::StdDevSizes => Box::new(ModuleStdDevSizes::new()),
            FeatureModules::TimeVelocity => Box::new(ModuleTimeVelocity::default()),
        }
    }
}

pub trait FeatureModule: Debug {
    fn name(&self) -> &str;
    fn value(&self) -> f64;
    fn update(&mut self, trade: &Trade, init: bool);
}

#[cfg(test)]
mod tests {
    use crate::Trade;

    pub const TRADES: [Trade; 10] = [
        Trade{ timestamp: 0, price: 100.0, size: 10.0 },
        Trade{ timestamp: 1, price: 101.0, size: -10.0 },
        Trade{ timestamp: 2, price: 100.0, size: 20.0 },
        Trade{ timestamp: 3, price: 102.0, size: 10.0 },
        Trade{ timestamp: 4, price: 103.0, size: 10.0 },
        Trade{ timestamp: 5, price: 104.0, size: -20.0 },
        Trade{ timestamp: 6, price: 102.0, size: -10.0 },
        Trade{ timestamp: 7, price: 101.0, size: 10.0 },
        Trade{ timestamp: 8, price: 102.0, size: 30.0 },
        Trade{ timestamp: 9, price: 105.0, size: 10.0 },
    ];
}
