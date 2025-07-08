use crate::MillisecondPeriod;

/// 1 Minute candle period
pub const M1: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(60);

/// 5 Minute candle period
pub const M5: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(300);

/// 15 Minute candle period
pub const M15: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(900);

/// 30 Minute candle period
pub const M30: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(1800);

/// 1 Hour candle period
pub const H1: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(3600);

/// 2 Hour candle period
pub const H2: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(7200);

/// 4 Hour candle period
pub const H4: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(14400);

/// 8 Hour candle period
pub const H8: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(28800);

/// 12 Hour candle period
pub const H12: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(43200);

/// 1 Day candle period
pub const D1: MillisecondPeriod = MillisecondPeriod::from_non_zero_secs(86400);
