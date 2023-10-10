//! This module contains a bunch of ready to use 'CandleComponents'
//! that can easily be combined to create a 'ModularCandle' using the 'Candle' macro.

mod average_price;
mod candle_component_trait;
mod close;
mod directional_trade_ratio;
mod directional_volume_ratio;
mod entropy;
mod high;
mod low;
mod median_price;
mod num_trades;
mod open;
#[cfg(feature = "chrono")]
mod open_datetime;
mod open_timestamp;
mod std_dev_prices;
mod std_dev_sizes;
mod time_velocity;
mod trades;
mod volume;
mod weighted_price;

pub use average_price::AveragePrice;
pub use candle_component_trait::{CandleComponent, CandleComponentUpdate};
pub use close::Close;
pub use directional_trade_ratio::DirectionalTradeRatio;
pub use directional_volume_ratio::DirectionalVolumeRatio;
pub use entropy::Entropy;
pub use high::High;
pub use low::Low;
pub use median_price::MedianPrice;
pub use num_trades::NumTrades;
pub use open::Open;
#[cfg(feature = "chrono")]
pub use open_datetime::OpenDateTime;
pub use open_timestamp::OpenTimeStamp;
pub use std_dev_prices::StdDevPrices;
pub use std_dev_sizes::StdDevSizes;
pub use time_velocity::TimeVelocity;
pub use trades::Trades;
pub use volume::Volume;
pub use weighted_price::WeightedPrice;

#[cfg(test)]
mod tests {
    use crate::Trade;

    pub const TRADES: [Trade; 10] = [
        Trade {
            timestamp: 1_684_677_200_000,
            price: 100.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1_684_677_210_000,
            price: 101.0,
            size: -10.0,
        },
        Trade {
            timestamp: 1_684_677_220_000,
            price: 100.0,
            size: 20.0,
        },
        Trade {
            timestamp: 1_684_677_230_000,
            price: 102.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1_684_677_240_000,
            price: 103.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1_684_677_250_000,
            price: 104.0,
            size: -20.0,
        },
        Trade {
            timestamp: 1_684_677_260_000,
            price: 102.0,
            size: -10.0,
        },
        Trade {
            timestamp: 1_684_677_270_000,
            price: 101.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1_684_677_280_000,
            price: 102.0,
            size: 30.0,
        },
        Trade {
            timestamp: 1_684_677_290_000,
            price: 105.0,
            size: 10.0,
        },
    ];
}
