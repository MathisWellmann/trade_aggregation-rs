#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Defines a taker trade
pub struct Trade {
    /// Timestamp, assumed to be in milliseconds
    pub timestamp: i64,

    /// Price of the asset
    pub price: f64,

    /// Size of the trade
    /// negative values indicate a taker Sell order
    pub size: f64,
}

impl TakerTrade for Trade {
    #[inline(always)]
    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    #[inline(always)]
    fn price(&self) -> f64 {
        self.price
    }

    #[inline(always)]
    fn size(&self) -> f64 {
        self.size
    }
}

/// Defines how to aggregate trade size
/// either by Base currency or Quote Currency
/// assumes trades sizes are denoted in Quote
/// e.g.: buy 10 contracts of BTC would be trade size of 10
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum By {
    /// when aggregating by Base, divide size by price for volume sum
    Base,
    /// when aggregating by Quote, take the raw trade size for volume sum
    /// as the assumption is that Trade size is denoted in Quote
    Quote,
}

/// Trait to enable third party types to be passed into aggregators.
pub trait TakerTrade {
    /// The timestamp of a trade,
    fn timestamp(&self) -> i64;

    /// units for the timestamp integer returned by [`TakerTrade.timestamp()`] method
    /// A default implementation is included and assumes milliseconds
    fn timestamp_resolution(&self) -> TimestampResolution {
        TimestampResolution::Millisecond
    }

    /// Fill price of the transaction
    fn price(&self) -> f64;

    /// Number of shares or contracts in this trade.
    /// A negative value indicates
    /// that the trade was a sell order, taking liquidity from the bid.
    fn size(&self) -> f64;
}

/// The resolution of the "TakerTrade" timestamps
#[derive(Debug, Clone, Copy)]
pub enum TimestampResolution {
    /// The timestamp of the TakerTrade is measured in milliseconds
    Millisecond,

    /// The timestamp of the TakerTrade is measured in microseconds
    Microsecond,

    /// The timestamp of the TakerTrade is measured in nanoseconds
    Nanosecond,
}

/// A period measured in milliseconds which must be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MillisecondPeriod(u64);

impl MillisecondPeriod {
    /// Try to create the `MillisecondPeriod` from millisecond units.
    /// # Panics:
    /// If `millis` is zero, the contract was violated.
    pub fn from_non_zero(millis: u64) -> Self {
        assert!(millis > 0, "`millis` must be non-zero that was the deal");
        Self(millis)
    }

    /// Try to create the `MillisecondPeriod` from seconds.
    /// # Panics:
    /// Because this is used in a const context, it is not yet possible to do `Option::unwrap` and thus
    /// it is asserted that `seconds` is non-zero
    pub const fn from_non_zero_secs(seconds: u64) -> Self {
        assert!(seconds > 0, "`seconds` must be non-zero that was the deal");
        Self(seconds * 1_000)
    }

    /// Get the inner value
    pub fn get(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn millisecond_period_from_non_zero_secs_panic() {
        // panics as it cannot be zero
        MillisecondPeriod::from_non_zero_secs(0);
    }

    #[test]
    fn millisecond_period_from_non_zero_secs() {
        assert_eq!(
            MillisecondPeriod::from_non_zero_secs(1),
            MillisecondPeriod(1_000)
        );
        assert_eq!(
            MillisecondPeriod::from_non_zero_secs(60),
            MillisecondPeriod(60_000)
        );
    }
}
