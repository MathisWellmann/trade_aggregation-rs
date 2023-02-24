#![deny(missing_docs, rustdoc::missing_crate_level_docs)]

//! This crate is used for aggregating raw trade data into candles using various methods

#[cfg(test)]
mod plot;

mod aggregation_rules;
mod aggregator;
pub mod candle_components;
mod constants;
mod errors;
mod modular_candle_trait;
mod types;
mod utils;
mod welford_online;

pub use aggregation_rules::*;
pub use aggregator::*;
pub use candle_components::{CandleComponent, CandleComponentUpdate};
pub use constants::*;
pub use errors::*;
pub use modular_candle_trait::ModularCandle;
pub use trade_aggregation_derive::Candle;
pub use types::*;
pub use utils::*;
