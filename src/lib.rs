#![deny(missing_docs, rustdoc::missing_crate_level_docs)]

//! This crate is used for aggregating raw trade data into candles using various methods

#[macro_use]
extern crate serde;

mod constants;
mod errors;
mod modular_volume_aggregator;
mod modules;
mod time_aggregator;
mod types;
mod utils;
mod volume_aggregator;
mod welford_online;

pub use constants::*;
pub use modular_volume_aggregator::ModularVolumeAggregator;
pub use modules::{FeatureModules, ModularCandle};
pub use time_aggregator::TimeAggregator;
pub use types::*;
pub use utils::*;
pub use volume_aggregator::VolumeAggregator;
