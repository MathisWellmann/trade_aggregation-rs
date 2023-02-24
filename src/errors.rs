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
