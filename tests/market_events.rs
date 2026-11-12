//! Candles over a stream that mixes trades and quotes.

use trade_aggregation::{
    candle_components::{Close, NumTrades, OpenTimeStamp},
    Aggregator, AlignedTimeRule, By, Candle, GenericAggregator, MarketEvent, MillisecondPeriod,
    Quote, Trade, VolumeRule,
};

type Event = MarketEvent<Trade, Quote>;

#[derive(Debug, Default, Clone, Candle)]
struct Mixed {
    close: Close,
    num_trades: NumTrades<u32>,
    open_ts: OpenTimeStamp<i64>,
}

fn trade(timestamp: i64, price: f64, size: f64) -> Event {
    Event::Trade(Trade {
        timestamp,
        price,
        size,
    })
}

fn quote(timestamp: i64, bid_price: f64, ask_price: f64) -> Event {
    Event::Quote(Quote {
        timestamp,
        bid_price,
        bid_size: 1.0,
        ask_price,
        ask_size: 1.0,
    })
}

#[test]
fn trade_rules_and_components_ignore_quotes() {
    let events = [
        quote(0, 99.0, 101.0),
        trade(1, 100.0, 1.0),
        quote(2, 99.0, 101.0),
        trade(3, 101.0, 1.0),
        quote(4, 100.0, 102.0),
        trade(5, 102.0, 1.0),
    ];
    let rule = VolumeRule::new(2.0, By::Quote).expect("valid threshold");
    let mut a = GenericAggregator::<Mixed, _, Event>::new(rule, false);
    let mut bars = Vec::new();
    for e in &events {
        a.update(e, &mut bars);
    }
    // The volume rule triggers on the second trade, which opens the next candle.
    assert_eq!(bars.len(), 1);
    let candle = bars[0].candle();
    assert_eq!(candle.num_trades(), 1);
    assert_eq!(candle.close(), 100.0);
    assert_eq!(
        candle.open_ts(),
        0,
        "the first event of any kind opens the candle"
    );
    assert_eq!(bars[0].num_events(), 3);
}

#[test]
fn time_rules_close_on_quotes_too() {
    let events = [trade(10, 100.0, 1.0), quote(120, 99.0, 101.0)];
    let rule = AlignedTimeRule::new(MillisecondPeriod::from_non_zero(100));
    let mut a = GenericAggregator::<Mixed, _, Event>::new(rule, false);
    let mut bars = Vec::new();
    for e in &events {
        a.update(e, &mut bars);
    }
    assert_eq!(bars.len(), 1);
    assert_eq!(bars[0].close_ts(), 100);
    assert_eq!(bars[0].candle().num_trades(), 1);
}
