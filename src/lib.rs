pub mod agg_time;
pub mod agg_time_streaming;
pub mod agg_volume;
pub mod agg_volume_streaming;
pub mod common;
pub mod agg_volume_streaming_modular;
pub mod modules;
mod welford_online;

pub use agg_time::agg_time;
pub use agg_time_streaming::AggTimeStreaming;
pub use agg_volume::agg_volume;
pub use agg_volume_streaming::AggVolumeStreaming;
pub use agg_volume_streaming_modular::AggVolumeStreamingModular;
pub use modules::FeatureModules;

/// Defines how to aggregate trade size
/// either by Base currency or Quote Currency
/// assumes trades sizes are denoted in Quote
/// e.g.: buy 10 contracts of BTC would be trade size of 10
#[derive(Debug, Clone)]
pub enum By {
    Base,
    Quote,
}
