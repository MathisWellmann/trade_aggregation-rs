#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
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
    fn timestamp(&self) -> i64 {
        self.timestamp
    }

    fn price(&self) -> f64 {
        self.price
    }

    fn size(&self) -> f64 {
        self.size
    }
}

/// Defines how to aggregate trade size
/// either by Base currency or Quote Currency
/// assumes trades sizes are denoted in Quote
/// e.g.: buy 10 contracts of BTC would be trade size of 10
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum By {
    /// when aggregating by Base, divide size by price for volume sum
    Base,
    /// when aggregating by Quote, take the raw trade size for volume sum
    /// as the assumption is that Trade size is denoted in Quote
    Quote,
}

/// Trait to enable third party types to be passed into aggregators.
pub trait TakerTrade {
    /// Timestamp, should be in milliseconds
    fn timestamp(&self) -> i64;

    /// Transaction price
    fn price(&self) -> f64;

    /// Number of shares or contracts in this trade.  A negative value indicates
    /// that the trade was executed on the bid (market sell order).
    fn size(&self) -> f64;
}
