/// Enumerate the possible errors in this crate
#[derive(thiserror::Error, Debug)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Csv(#[from] csv::Error),

    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),

    #[error(transparent)]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("An invalid parameter was provided")]
    InvalidParam,
}

/// Convenient wrapper for this crates custom Error
pub type Result<T> = std::result::Result<T, Error>;

/// An event or a watermark that is earlier than the latest time an aggregator has seen,
/// e.g. a late event of a live stream.
///
/// It is a type of its own and not a variant of [`Error`], so that the caller can match it
/// without the other variants, and so that adding it did not break matches on [`Error`].
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("out of time order: {timestamp} is before the latest time {latest}")]
pub struct OutOfOrder {
    timestamp: i64,
    latest: i64,
}

impl OutOfOrder {
    pub(crate) fn new(timestamp: i64, latest: i64) -> Self {
        assert!(timestamp < latest, "{timestamp} is in order after {latest}");
        Self { timestamp, latest }
    }

    /// The time of the rejected event or watermark.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    /// The latest time the aggregator had seen.
    pub fn latest(&self) -> i64 {
        self.latest
    }
}
