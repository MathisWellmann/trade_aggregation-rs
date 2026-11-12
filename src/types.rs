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

impl Timestamped for Trade {
    const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Millisecond;

    #[inline(always)]
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl TakerTrade for Trade {
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

/// An event with a timestamp, e.g. a trade or a quote.
pub trait Timestamped {
    /// The unit of the integer returned by [`Self::timestamp`].
    ///
    /// It is a constant of the type, so that rules on a clock know it before the first event,
    /// and it has no default, because a wrong unit silently scales every period.
    const TIMESTAMP_RESOLUTION: TimestampResolution;

    /// The timestamp of the event, in units of [`Self::TIMESTAMP_RESOLUTION`].
    fn timestamp(&self) -> i64;
}

/// Trait to enable third party trade types to be passed into aggregators.
pub trait TakerTrade: Timestamped {
    /// Fill price of the transaction
    fn price(&self) -> f64;

    /// Number of shares or contracts in this trade.
    /// A negative value indicates
    /// that the trade was a sell order, taking liquidity from the bid.
    fn size(&self) -> f64;
}

/// The resolution of the [`Timestamped`] timestamps
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TimestampResolution {
    /// The timestamp of the TakerTrade is measured in milliseconds
    Millisecond,

    /// The timestamp of the TakerTrade is measured in microseconds
    Microsecond,

    /// The timestamp of the TakerTrade is measured in nanoseconds
    Nanosecond,
}

impl TimestampResolution {
    /// The number of timestamp units in one millisecond.
    pub(crate) const fn units_per_millisecond(self) -> i64 {
        match self {
            Self::Millisecond => 1,
            Self::Microsecond => 1_000,
            Self::Nanosecond => 1_000_000,
        }
    }
}

/// A period measured in milliseconds which must be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "u64", into = "u64"))]
pub struct MillisecondPeriod(u64);

impl TryFrom<u64> for MillisecondPeriod {
    type Error = crate::Error;

    /// # Errors:
    /// [`Error::InvalidParam`](crate::Error::InvalidParam) if `millis` is zero.
    fn try_from(millis: u64) -> crate::Result<Self> {
        if millis == 0 {
            return Err(crate::Error::InvalidParam);
        }
        Ok(Self(millis))
    }
}

impl From<MillisecondPeriod> for u64 {
    fn from(period: MillisecondPeriod) -> Self {
        period.0
    }
}

impl MillisecondPeriod {
    /// Try to create the `MillisecondPeriod` from millisecond units.
    /// # Panics:
    /// If `millis` is zero, the contract was violated.
    pub const fn from_non_zero(millis: u64) -> Self {
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

    /// The period in timestamp units of the given resolution,
    /// saturating at `i64::MAX` for periods no trade data can span.
    pub(crate) fn in_units(self, resolution: TimestampResolution) -> i64 {
        let units = i64::try_from(self.0)
            .unwrap_or(i64::MAX)
            .saturating_mul(resolution.units_per_millisecond());
        debug_assert!(units > 0);
        units
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
    fn millisecond_period_try_from_rejects_zero() {
        assert!(MillisecondPeriod::try_from(0).is_err());
        assert_eq!(
            MillisecondPeriod::try_from(5).expect("non-zero"),
            MillisecondPeriod(5)
        );
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
