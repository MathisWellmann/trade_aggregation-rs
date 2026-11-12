//! The `Candle` derive, used from outside the crate without importing its traits.

use trade_aggregation::{
    candle_components::{Close, NumTrades, Open, Trades},
    Aggregator, Candle, CandleComponent, CandleComponentUpdate, GenericAggregator, TickRule, Trade,
};

#[derive(Debug, Default, Clone, Candle)]
#[candle(record = OhlcRecord, record_derive(Debug, Clone, Copy, PartialEq))]
struct Ohlc {
    open: Open,
    close: Close,
    num_trades: NumTrades<u32>,
}

/// Remembers the time at which its candle closed.
#[derive(Debug, Default, Clone)]
struct ClosedAt {
    at: Option<i64>,
}

impl CandleComponent for ClosedAt {
    type Value = Option<i64>;

    fn value(&self) -> Option<i64> {
        self.at
    }

    fn reset(&mut self) {
        self.at = None;
    }
}

impl<E> CandleComponentUpdate<E> for ClosedAt {
    fn update(&mut self, _event: &E) {}

    fn close_at(&mut self, timestamp: i64) {
        self.at = Some(timestamp);
    }
}

/// A candle for any event type.
#[derive(Debug, Default, Clone, Candle)]
struct Stamped {
    closed_at: ClosedAt,
}

/// A candle with a generic parameter.
#[derive(Debug, Default, Clone, Candle)]
struct Collected<T: trade_aggregation::TakerTrade + Clone + Default> {
    trades: Trades<T>,
}

fn trades(n: i64) -> Vec<Trade> {
    (0..n)
        .map(|i| Trade {
            timestamp: i * 10,
            price: 100.0 + i as f64,
            size: 1.0,
        })
        .collect()
}

#[test]
fn the_derive_lists_the_fields_and_builds_a_record() {
    assert_eq!(Ohlc::FIELDS, ["open", "close", "num_trades"]);
    let mut a = GenericAggregator::<Ohlc, TickRule, Trade>::new(TickRule::new(3), false);
    let mut bars = Vec::new();
    for t in &trades(4) {
        a.update(t, &mut bars);
    }
    let record = bars[0].candle().record();
    assert_eq!(
        record,
        OhlcRecord {
            open: 100.0,
            close: 101.0,
            num_trades: 2,
        }
    );
}

#[test]
fn close_at_reaches_the_components() {
    let mut a = GenericAggregator::<Stamped, TickRule, Trade>::new(TickRule::new(3), false);
    let mut bars = Vec::new();
    for t in &trades(4) {
        a.update(t, &mut bars);
    }
    assert_eq!(bars[0].candle().closed_at(), Some(20));
    let provisional = a.provisional(35).expect("a candle is open");
    assert_eq!(provisional.candle().closed_at(), Some(35));
    assert_eq!(
        a.unfinished_candle().closed_at(),
        None,
        "a provisional bar does not change the unfinished candle"
    );
}

#[test]
fn a_generic_candle_struct_derives() {
    let mut a =
        GenericAggregator::<Collected<Trade>, TickRule, Trade>::new(TickRule::new(2), false);
    let mut bars = Vec::new();
    for t in &trades(3) {
        a.update(t, &mut bars);
    }
    assert_eq!(bars[0].candle().trades().len(), 1);
    assert_eq!(Collected::<Trade>::FIELDS, ["trades"]);
}
