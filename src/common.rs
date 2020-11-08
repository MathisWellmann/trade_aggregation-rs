use std::fs::File;
use chrono::naive::NaiveDateTime;


pub const M1: i64 = 60;  // 1 minute candle constant
pub const M5: i64 = 300;
pub const M15: i64 = 900;
pub const M30: i64 = 1800;
pub const H1: i64 = 3600;  // 1 hour candle constant
pub const H2: i64 = 7200;
pub const H4: i64 = 14400;
pub const H8: i64 = 28800;
pub const H12: i64 = 43200;
pub const D1: i64 = 86400;  // 1 day candle constant

#[derive(Debug, Clone)]
pub struct Trade {
    pub timestamp: i64,
    pub price: f64,
    pub size: f64,
}

#[derive(Debug, Clone)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub volume_direction_ratio: f64,  // buy_volume / volume // in range [0, 1]
    pub trade_direction_ratio: f64,  // num_buys / num_trades // in range [0, 1]
    pub num_trades: i32,
    pub weighted_price: f64,
    pub std_dev_prices: f64,
    pub std_dev_sizes: f64,
}

impl std::fmt::Display for Candle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "(ts: {:?}, o: {:.2}, h: {:.2}, l: {:.2}, c: {:.2}, wp: {:.2}, v: {:.2}, vdr: {:.2}, tdr: {:.2}, #t: {}, σ_price: {:.2}, σ_size: {:.2})",
               NaiveDateTime::from_timestamp(self.timestamp / 1000, (self.timestamp % 1000) as u32),
               self.open,
               self.high,
               self.low,
               self.close,
               self.weighted_price,
               self.volume,
               self.volume_direction_ratio,
               self.trade_direction_ratio,
               self.num_trades,
               self.std_dev_prices,
               self.std_dev_sizes,
        )
    }
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
    };
    return out
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn candle_display() {
        let c = Candle {
            timestamp: 1591889593548,
            open: 9565.0,
            high: 9566.5,
            low: 9555.0,
            close: 9555.0,
            volume: 6.500301656683413,
            volume_direction_ratio: 0.042005987543157944,
            trade_direction_ratio: 0.0,
            num_trades: 58,
            weighted_price: 9556.479572933373,
            std_dev_prices: 3.953000116537345,
            std_dev_sizes: 6565.830432012996,
        };
        println!("c: {}", c);
        assert!(false);
    }
}