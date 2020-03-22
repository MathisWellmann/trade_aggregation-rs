use mongodb::*;
use bson::ordered::OrderedDocument;
use chrono::prelude::*;

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

// get_trades will query all trades from database and return a vector of type Trade
pub fn get_trades() -> Vec<Trade> {
    // get trades from database
    let dbpath = "mongodb://192.168.1.32:27017";

    let client = Client::with_uri_str(dbpath).expect("failed to connect to database");
    let db = client.database("Bitmex");
    let coll = db.collection("XBTM20-Trades");

    let cursor = coll.find(None, None).ok().expect("failed to find");

    let mut trades: Vec<Trade> = Vec::new();
    for x in cursor {
        match x {
            Ok(val) => {
                let mut t = Trade{
                    timestamp: 0,
                    price: 0.0,
                    size: 0.0
                };
                if let Some(ts) = OrderedDocument::get_str(&val, "Timestamp").ok() {
                    let timestamp = ts.parse::<DateTime<Utc>>().unwrap();
                    t.timestamp = timestamp.timestamp_millis();
                }
                if let Some(p) = OrderedDocument::get_f64(&val, "Price").ok() {
                    t.price = p;
                };
                if let Some(s) = OrderedDocument::get_f64(&val, "Size").ok() {
                    t.size = s;
                };
                // println!("t: {:#?}", t);
                trades.push(t);
            }
            Err(err) => {
                println!("Err: {}", err)
            }
        }
    };
    return trades
}
