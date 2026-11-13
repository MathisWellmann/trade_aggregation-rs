#![doc = include_str!("../README.md")]

//! This crate is used for aggregating raw trade data into candles using various methods

// The `Candle` derive refers to `::trade_aggregation`, also within this crate.
extern crate self as trade_aggregation;

#[cfg(test)]
mod plot;

mod aggregation_rules;
mod aggregator;
mod bar;
pub mod candle_components;
mod constants;
mod errors;
mod market_event;
mod modular_candle_trait;
mod price_source;
mod types;
mod utils;
mod welford_online;

pub use aggregation_rules::*;
pub use aggregator::*;
pub use bar::{Bar, BarKind};
pub use candle_components::{CandleComponent, CandleComponentUpdate};
pub use constants::*;
pub use errors::*;
pub use market_event::{AsQuote, AsTrade, BestQuote, MarketEvent, Quote};
pub use modular_candle_trait::ModularCandle;
pub use price_source::{Microprice, MidPrice, PriceKind, PriceSource, TradePrice};
pub use trade_aggregation_derive::Candle;
pub use types::*;
pub use utils::*;
