//! This example shows how to implement the TakerTrade trait for your own
//! input type.  It builds on the `aggregate_all_ohlc.rs` example. However, instead
//! of using the default `Trade` type we define a custom type called `Tick`,
//! which is very similar to `Trade`.  Take note of the `input` field on
//! `MyCandle` and the use of `PhantomData`, the `Candle` derive macro parses
//! the `MyCandle` struct definition for this attribute and uses it to define the generic
//! trait `ModularCandle<T>`.

//! When deriving the 'Candle' macro, make sure the following things are in scope:
//! - Trade
//! - ModularCandle
//! - CandleComponent

use std::marker::PhantomData;

use trade_aggregation::{
    candle_components::{Close, High, Low, Open},
    *,
};

pub enum Side {
    Bid,
    Ask,
}

pub struct Tick {
    /// Timestamp, assumed to be in milliseconds
    pub date_stamp: i64,

    /// Price of the asset
    pub trade_price: f64,

    /// Size of the trade
    pub size: usize,
    /// whether the trade was executed on the bid or the ask
    pub side: Side,
}

impl TakerTrade for Tick {
    #[inline(always)]
    fn timestamp(&self) -> i64 {
        self.date_stamp
    }

    #[inline(always)]
    fn price(&self) -> f64 {
        self.trade_price
    }

    #[inline(always)]
    fn size(&self) -> f64 {
        match self.side {
            Side::Bid => self.size as f64 * -1.0,
            Side::Ask => self.size as f64,
        }
    }
}

impl From<Trade> for Tick {
    fn from(trade: Trade) -> Self {
        let side = if trade.size > 0.0 {
            Side::Ask
        } else {
            Side::Bid
        };
        Tick {
            date_stamp: trade.timestamp,
            trade_price: trade.price,
            size: trade.size.abs() as usize,
            side,
        }
    }
}

#[derive(Debug, Default, Clone, Candle)]
struct MyCandle {
    open: Open,
    high: High,
    low: Low,
    close: Close,
    input: PhantomData<Tick>,
}

fn main() {
    let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
        .expect("Could not load trades from file!");
    let ticks: Vec<Tick> = trades.into_iter().map(|x| x.into()).collect();

    // specify the aggregation rule to be time based
    let time_rule = TimeRule::new(M1, TimestampResolution::Millisecond);
    let mut aggregator = GenericAggregator::<MyCandle, TimeRule, Tick>::new(time_rule);

    let candles = aggregate_all_trades(&ticks, &mut aggregator);
    println!("got {} candles", candles.len());
}
