#[derive(Debug)]
pub struct Trade {
    pub(crate) timestamp: i64,
    pub(crate) price: f64,
    pub(crate) size: f64,
}

#[derive(Debug)]
pub struct Candle {
    pub(crate) timestamp: i64,
    pub(crate) open: f64,
    pub(crate) high: f64,
    pub(crate) low: f64,
    pub(crate) close: f64,
    pub(crate) volume: f64,
    pub(crate) volume_direction_ratio: f64,
    pub(crate) trade_direction_ratio: f64,
    pub(crate) num_trades: i8,
    pub(crate) weighted_price: f64,
}
