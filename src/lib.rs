#![deny(missing_docs, missing_crate_level_docs)]

//! This crate is used for aggregating raw trade data into candles using various methods

#[macro_use]
extern crate serde;

use chrono::naive::NaiveDateTime;
use std::fs::File;

mod time_aggregator;
mod volume_aggregator;
mod modular_volume_aggregator;
mod modules;
mod welford_online;

pub use time_aggregator::TimeAggregator;
pub use volume_aggregator::VolumeAggregator;
pub use modular_volume_aggregator::ModularVolumeAggregator;
pub use modules::{FeatureModules, ModularCandle};

/// 1 Minute candle period
pub const M1: i64 = 60;
/// 5 Minute candle period
pub const M5: i64 = 300;
/// 15 Minute candle period
pub const M15: i64 = 900;
/// 30 Minute candle period
pub const M30: i64 = 1800;
/// 1 Hour candle period
pub const H1: i64 = 3600;
/// 2 Hour candle period
pub const H2: i64 = 7200;
/// 4 Hour candle period
pub const H4: i64 = 14400;
/// 8 Hour candle period
pub const H8: i64 = 28800;
/// 12 Hour candle period
pub const H12: i64 = 43200;
/// 1 Day candle period
pub const D1: i64 = 86400;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
/// Defines a taker trade
pub struct Trade {
    /// Timestamp usually denoted in milliseconds
    pub timestamp: i64,
    /// Price of the asset
    pub price: f64,
    /// Size of the trade
    /// negative values indicate a taker Sell order
    pub size: f64,
}

#[derive(Debug, Clone)]
/// Defines a Candle
pub struct Candle {
    /// latest timestamp of last received trade
    pub timestamp: i64,
    /// open price of candle
    pub open: f64,
    /// high price of candle
    pub high: f64,
    /// low price of candle
    pub low: f64,
    /// close price of candle
    pub close: f64,
    /// summed taker volume of all trades in candle
    pub volume: f64,
    /// #buys / #trades
    pub directional_trade_ratio: f64,
    /// buy_volume / volume
    pub directional_volume_ratio: f64,  // buy_volume / volume // in range [0, 1]
    /// number of taker trades observed in candle
    pub num_trades: i32,
    /// arithmetic mean of price
    pub arithmetic_mean_price: f64,
    /// volume weighted price
    pub weighted_price: f64,
    /// standard deviation of trade prices
    pub std_dev_prices: f64,
    /// standard deviation of trade sizes
    pub std_dev_sizes: f64,
    /// measure of candle creation time: 1.0 / time_in_seconds
    pub time_velocity: f64,
}

impl std::fmt::Display for Candle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(ts: {:?}, o: {:.8}, h: {:.8}, l: {:.8}, c: {:.8}, wp: {:.8}, v: {:.2}, dtr: {:.4}, dvr: {:.4}, #t: {}, σ_price: {:.4}, σ_size: {:.4}, tv: {:.4})",
               NaiveDateTime::from_timestamp(self.timestamp / 1000, (self.timestamp % 1000) as u32),
               self.open,
               self.high,
               self.low,
               self.close,
               self.weighted_price,
               self.volume,
               self.directional_trade_ratio,
               self.directional_volume_ratio,
               self.num_trades,
               self.std_dev_prices,
               self.std_dev_sizes,
               self.time_velocity,
        )
    }
}

/// Defines how to aggregate trade size
/// either by Base currency or Quote Currency
/// assumes trades sizes are denoted in Quote
/// e.g.: buy 10 contracts of BTC would be trade size of 10
#[derive(Debug, Clone, Copy)]
pub enum By {
    /// when aggregating by Base, divide size by price for volume sum
    Base,
    /// when aggregating by Quote, take the raw trade size for volume sum
    /// as the assumption is that Trade size is denoted in Quote
    Quote,
}

/// Defines the needed methods for any online aggregator
pub trait Aggregator {
    /// Adds a new trade to aggregation
    /// Returns Some(Candle) only when a new candle has been created,
    /// otherwise it returns None
    fn update(&mut self, trade: &Trade) -> Option<Candle>;
}

/** Determine the candle volume which produces the same number of candles
as the given time aggregation equivalent
# Parameters:
- total_volume - sum of traded volume over entire time period
- total_time_days - total number of days
- target_time_minutes - time aggregated candle period which to target
# Returns:
- target candle volume for which volume aggregation produces
the same number of candles as the time aggregation did
e.g.:
10 days of 1h candle -> 240 candles
assuming 9840 volume traded over 10 days
-> each candle should have 41 volume to produce 240 candles using volume aggregation
**/
pub fn candle_volume_from_time_period(
    total_volume: f64,
    total_time_days: f64,
    target_time_minutes: f64,
) -> f64 {
    let num_candles = total_time_days * 24.0 * (60.0 / target_time_minutes);
    total_volume / num_candles
}

/// apply an aggregator for all trades at once
pub fn aggregate_all_trades(
    trades: &Vec<Trade>,
    aggregator: &mut impl Aggregator
) -> Vec<Candle> {
    let mut out: Vec<Candle> = vec![];

    for t in trades {
        match aggregator.update(t) {
            Some(candle) => out.push(candle),
            None => {}
        }
    }

    out
}

/// load trades from csv file
pub fn load_trades_from_csv(filename: &str) -> Vec<Trade> {
    let f = File::open(filename).unwrap();

    let mut r = csv::Reader::from_reader(f);

    let mut out: Vec<Trade> = vec![];
    for record in r.records() {
        let row = record.unwrap();

        let ts = row[0].parse::<i64>().unwrap();
        let price = row[1].parse::<f64>().unwrap();
        let size = row[2].parse::<f64>().unwrap();
        // convert to Trade
        let trade = Trade{
            timestamp: ts,
            price,
            size,
        };
        out.push(trade);
    };
    return out
}

#[cfg(test)]
mod tests {
    use super::*;
    use round::round;
    use crate::By;

    /// test_candle will assert if the candle violates any constraints
    pub fn test_candle(candle: &Candle) {
        assert!(candle.open <= candle.high);
        assert!(candle.open >= candle.low);
        assert!(candle.high >= candle.low);
        assert!(candle.close <= candle.high);
        assert!(candle.close >= candle.low);
        assert!(candle.volume > 0.0);
        assert!(candle.weighted_price <= candle.high);
        assert!(candle.weighted_price >= candle.low);
        assert!(candle.timestamp > 0);
        assert!(candle.directional_volume_ratio <= 1.0);
        assert!(candle.directional_volume_ratio >= 0.0);
        assert!(candle.directional_trade_ratio <= 1.0);
        assert!(candle.directional_trade_ratio >= 0.0);
        assert!(candle.num_trades > 0);
    }

    #[test]
    fn test_aggregate_all_trades() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let mut aggregator = VolumeAggregator::new(
            100.0,
            By::Quote
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert!(candles.len() > 0);
    }

    #[test]
    fn test_candle_volume_from_time_period() {
        let total_volume = 100.0;
        let time_days = 10.0;
        let target_time_minutes = 5.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.035);

        let total_volume = 100.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.069);

        let total_volume = 200.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.139);

        let total_volume = 50.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.035);

        let total_volume = 100.0;
        let time_days = 5.0;
        let target_time_minutes = 5.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.069);

        let total_volume = 100.0;
        let time_days = 5.0;
        let target_time_minutes = 10.0;
        let vol_threshold = candle_volume_from_time_period(
            total_volume,
            time_days,
            target_time_minutes
        );
        assert_eq!(round(vol_threshold, 3), 0.139);
    }

    #[test]
    fn candle_display() {
        let c = Candle {
            timestamp: 1591889593548,
            open: 9565.0,
            high: 9566.5,
            low: 9555.0,
            close: 9555.0,
            volume: 6.500301656683413,
            directional_volume_ratio: 0.042005987543157944,
            directional_trade_ratio: 0.0,
            num_trades: 58,
            arithmetic_mean_price: 9556.0,
            weighted_price: 9556.479572933373,
            std_dev_prices: 3.953000116537345,
            std_dev_sizes: 6565.830432012996,
            time_velocity: 0.1,
        };
        println!("c: {}", c);
    }
}
