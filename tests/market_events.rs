//! Candles over a stream that mixes trades and quotes.

use trade_aggregation::{
    candle_components::{
        BipowerVariation, Close, EffectiveSpread, MidClose, NumTrades, OpenTimeStamp, QuoteUpdates,
        RealizedQuarticity, RealizedVariance, TimeWeightedSpread,
    },
    Aggregator, AlignedTimeRule, By, Candle, GenericAggregator, MarketEvent, MillisecondPeriod,
    ModularCandle, Quote, Trade, VolumeRule,
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

/// The statistics of the path of the trade prices.
#[derive(Debug, Default, Clone, Candle)]
struct TradePath {
    rv: RealizedVariance,
    bv: BipowerVariation,
    rq: RealizedQuarticity,
}

#[test]
fn trade_path_statistics_ignore_quotes() {
    let events = [
        quote(0, 90.0, 120.0),
        trade(1, 100.0, 1.0),
        quote(2, 90.0, 120.0),
        trade(3, 101.0, 1.0),
        quote(4, 80.0, 130.0),
        trade(5, 99.0, -1.0),
        trade(6, 102.0, 1.0),
    ];
    let mut mixed = TradePath::default();
    let mut trades_only = TradePath::default();
    for e in &events {
        ModularCandle::update(&mut mixed, e);
        if let Event::Trade(t) = e {
            ModularCandle::update(&mut trades_only, t);
        }
    }
    assert!(mixed.rv() > 0.0);
    assert!(mixed.bv() > 0.0);
    assert!(mixed.rq() > 0.0);
    // The quotes are far from the trades, so a return to a quote would change the values.
    assert_eq!(mixed.rv(), trades_only.rv());
    assert_eq!(mixed.bv(), trades_only.bv());
    assert_eq!(mixed.rq(), trades_only.rq());

    let mut quotes_only = TradePath::default();
    for e in events.iter().filter(|e| matches!(e, Event::Quote(_))) {
        ModularCandle::update(&mut quotes_only, e);
    }
    assert_eq!(quotes_only.rv(), 0.0);
    assert_eq!(quotes_only.bv(), 0.0);
    assert_eq!(quotes_only.rq(), 0.0);
}

/// Trade and quote statistics over the same interval.
#[derive(Debug, Default, Clone, Candle)]
#[candle(record = FusedRecord, record_derive(Debug, Clone, Copy, PartialEq))]
struct Fused {
    close: Close,
    num_trades: NumTrades<u32>,
    mid_close: MidClose,
    quote_updates: QuoteUpdates,
    spread: TimeWeightedSpread,
    effective_spread: EffectiveSpread,
}

#[test]
fn one_candle_holds_trade_and_quote_statistics() {
    let events = [
        quote(0, 99.0, 101.0),
        trade(10, 101.0, 1.0),
        quote(50, 98.0, 102.0),
        trade(60, 98.0, 1.0),
        quote(150, 99.0, 101.0),
    ];
    let rule = AlignedTimeRule::new(MillisecondPeriod::from_non_zero(100));
    let mut a = GenericAggregator::<Fused, _, Event>::new(rule, false);
    let mut bars = Vec::new();
    for e in &events {
        a.update(e, &mut bars);
    }
    assert_eq!(bars.len(), 1);
    let record = bars[0].candle().record();
    assert_eq!(record.close, 98.0);
    assert_eq!(record.num_trades, 2);
    assert_eq!(record.mid_close, Some(100.0));
    assert_eq!(record.quote_updates, 2);
    // A relative spread of 0.02 for 50 ms and 0.04 until the close at 100.
    let spread = record.spread.expect("quotes");
    assert!((spread - 0.03).abs() < 1e-15, "{spread}");
    // Both trades are 1 or 2 away from a mid of 100, with the same base size (1 / price).
    let effective = record.effective_spread.expect("trades");
    let (w1, w2) = (1.0 / 101.0, 1.0 / 98.0);
    let expected = (w1 * 0.02 + w2 * 0.04) / (w1 + w2);
    assert!((effective - expected).abs() < 1e-15, "{effective}");

    // The next candle starts with the quote in effect at 100.
    let provisional = a.provisional(150).expect("a candle is open");
    let spread = provisional.candle().spread().expect("quotes");
    assert!((spread - 0.04).abs() < 1e-15, "{spread}");
}
