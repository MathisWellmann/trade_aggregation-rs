//! This module contains a bunch of ready to use 'CandleComponents'
//! that can easily be combined to create a 'ModularCandle' using the 'Candle' macro.

mod average_price;
mod base_volume;
mod candle_component_trait;
mod close;
mod close_timestamp;
mod directional_trade_ratio;
mod directional_volume_ratio;
mod entropy;
mod high;
mod largest_trade;
mod low;
mod median_price;
mod num_trades;
mod open;
#[cfg(feature = "chrono")]
mod open_datetime;
mod open_timestamp;
mod order_flow;
mod quote_volume;
mod quotes;
mod realized;
mod return_path;
mod returns;
mod std_dev_prices;
mod std_dev_sizes;
mod taker_orders;
mod time_velocity;
mod trade_signs;
mod trade_sizes;
mod trades;
mod two_scale;
mod volume;
mod volume_buys;
mod volume_sells;
mod weighted_price;

pub use average_price::AveragePrice;
pub use base_volume::BaseVolume;
pub use candle_component_trait::{CandleComponent, CandleComponentUpdate};
pub use close::Close;
pub use close_timestamp::CloseTimeStamp;
pub use directional_trade_ratio::DirectionalTradeRatio;
pub use directional_volume_ratio::DirectionalVolumeRatio;
pub use entropy::Entropy;
pub use high::High;
pub use largest_trade::LargestTrade;
pub use low::Low;
pub use median_price::MedianPrice;
pub use num_trades::NumTrades;
pub use open::Open;
#[cfg(feature = "chrono")]
pub use open_datetime::OpenDateTime;
pub use open_timestamp::OpenTimeStamp;
pub use order_flow::{
    BuyVwap, KyleLambda, KyleLambdaStats, NetFlow, NetFlowHigh, NetFlowLow, RegressionSums,
    SellVwap,
};
pub use quote_volume::QuoteVolume;
pub use quotes::{
    EffectiveSpread, MidClose, MidHigh, MidLow, MidOpen, OrderFlowImbalance, QuoteUpdates,
    TimeWeightedImbalance, TimeWeightedSpread, WapRealizedVariance,
};
pub use realized::{BipowerVariation, RealizedQuarticity, RealizedVariance};
pub use return_path::{
    roll_spread, EfficiencyRatio, MaxDrawdown, MaxRunup, ReturnAutocovariance, TimeOfHigh,
    TimeOfLow,
};
pub use returns::{
    realized_kurtosis, realized_skewness, DownTicks, MaxAbsReturn, NegativeSemivariance,
    PositiveSemivariance, RealizedCubic, RealizedQuartic, ReturnCount, TotalVariation,
    UnchangedTicks, UpTicks,
};
pub use std_dev_prices::StdDevPrices;
pub use std_dev_sizes::StdDevSizes;
pub use taker_orders::{MaxSweepDepth, Sweeps, TakerOrders};
pub use time_velocity::TimeVelocity;
pub use trade_signs::{ConditionalSignEntropy, LongestRun, SignAutocorrelation, SignSwitches};
pub use trade_sizes::{
    LargeTradeFlow, LargestTradeShare, MediumTradeFlow, SizeHerfindahl, SmallTradeFlow,
};
pub use trades::Trades;
pub use two_scale::TwoScaleRealizedVariance;
pub use volume::Volume;
pub use volume_buys::VolumeBuys;
pub use volume_sells::VolumeSells;
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
