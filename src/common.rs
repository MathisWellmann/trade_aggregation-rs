use std::fs::File;

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
    pub(crate) num_trades: i32,
    pub(crate) weighted_price: f64,
}

// test_candle will assert if the candle violates any constraints
pub fn test_candle(candle: &Candle) {
    assert!(candle.open <= candle.high);
    assert!(candle.open >= candle.low);
    assert!(candle.high >= candle.low);
    assert!(candle.close <= candle.high);
    assert!(candle.close >= candle.low);
    assert!(candle.volume > 0.0);
    assert!(candle.weighted_price <= candle.high);
    assert!(candle.weighted_price >= candle.low);
    assert!(candle.timestamp > 0);
    assert!(candle.volume_direction_ratio <= 1.0);
    assert!(candle.volume_direction_ratio >= 0.0);
    assert!(candle.trade_direction_ratio <= 1.0);
    assert!(candle.trade_direction_ratio >= 0.0);
    assert!(candle.num_trades > 0);
}

pub fn load_trades_from_csv(filename: &str) -> Vec<Trade> {
    let f = File::open(filename).unwrap();

    let mut r = csv::Reader::from_reader(f);

    let mut out: Vec<Trade> = vec![];
    let mut count: i64 = 0;
    for record in r.records() {
        let row = record.unwrap();

        let ts = row[0].parse::<i64>().unwrap();
        let price = row[1].parse::<f64>().unwrap();
        let size = row[2].parse::<f64>().unwrap();
        // convert to Trade
        let trade = Trade{
            timestamp: ts,
            price: price,
            size: size,
        };
        out.push(trade);
        count += 1;
    };
    return out
}
