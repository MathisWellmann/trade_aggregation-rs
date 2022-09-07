//! This module contains a bunch of ready to use 'CandleComponents'
//! that can easily be combined to create a 'ModularCandle' using the 'Candle' macro.

mod average_price;
mod candle_component_trait;
mod close;
mod directional_trade_ratio;
mod directional_volume_ratio;
mod high;
mod low;
mod median_price;
mod num_trades;
mod open;
mod std_dev_prices;
mod std_dev_sizes;
mod time_velocity;
mod volume;
mod weighted_price;

pub use average_price::AveragePrice;
pub use candle_component_trait::CandleComponent;
pub use candle_component_trait::CandleComponentUpdate;
pub use close::Close;
pub use directional_trade_ratio::DirectionalTradeRatio;
pub use directional_volume_ratio::DirectionalVolumeRatio;
pub use high::High;
pub use low::Low;
pub use median_price::MedianPrice;
pub use num_trades::NumTrades;
pub use open::Open;
pub use std_dev_prices::StdDevPrices;
pub use std_dev_sizes::StdDevSizes;
pub use time_velocity::TimeVelocity;
pub use volume::Volume;
pub use weighted_price::WeightedPrice;

#[cfg(test)]
mod tests {
    use crate::Trade;

    pub const TRADES: [Trade; 10] = [
        Trade {
            timestamp: 0,
            price: 100.0,
            size: 10.0,
        },
        Trade {
            timestamp: 1,
            price: 101.0,
            size: -10.0,
        },
        Trade {
            timestamp: 2,
            price: 100.0,
            size: 20.0,
        },
        Trade {
            timestamp: 3,
            price: 102.0,
            size: 10.0,
        },
        Trade {
            timestamp: 4,
            price: 103.0,
            size: 10.0,
        },
        Trade {
            timestamp: 5,
            price: 104.0,
            size: -20.0,
        },
        Trade {
            timestamp: 6,
            price: 102.0,
            size: -10.0,
        },
        Trade {
            timestamp: 7,
            price: 101.0,
            size: 10.0,
        },
        Trade {
            timestamp: 8,
            price: 102.0,
            size: 30.0,
        },
        Trade {
            timestamp: 9,
            price: 105.0,
            size: 10.0,
        },
    ];
}
