//! Return features in a derived candle: empty candles of a time rule and a mixed stream.

use trade_aggregation::{
    candle_components::{
        DownTicks, EfficiencyRatio, MaxAbsReturn, NegativeSemivariance, PositiveSemivariance,
        RealizedCubic, RealizedQuartic, RealizedVariance, ReturnAutocovariance, ReturnCount,
        TotalVariation, UnchangedTicks, UpTicks,
    },
    AlignedTimeRule, Gaps, GenericAggregator, MarketEvent, MidPrice, MillisecondPeriod,
    ModularCandle, Quote, Trade,
};

type Event = MarketEvent<Trade, Quote>;

/// The return sums of the mid price, for a stream of trades and quotes.
#[derive(Debug, Default, Clone, trade_aggregation::Candle)]
#[candle(record = SumsRecord, record_derive(Debug, Clone, Copy, PartialEq))]
struct Sums {
    n: ReturnCount<MidPrice>,
    up: UpTicks<MidPrice>,
    down: DownTicks<MidPrice>,
    flat: UnchangedTicks<MidPrice>,
    rv: RealizedVariance<MidPrice>,
    rs_up: PositiveSemivariance<MidPrice>,
    rs_down: NegativeSemivariance<MidPrice>,
    cubic: RealizedCubic<MidPrice>,
    quartic: RealizedQuartic<MidPrice>,
    tv: TotalVariation<MidPrice>,
    max_abs: MaxAbsReturn<MidPrice>,
}

/// The shape of the path of the mid price.
#[derive(Debug, Default, Clone, trade_aggregation::Candle)]
#[candle(record = PathRecord, record_derive(Debug, Clone, Copy, PartialEq))]
struct Path {
    efficiency: EfficiencyRatio<MidPrice>,
    autocovariance: ReturnAutocovariance<MidPrice>,
}

fn trade(timestamp: i64, price: f64) -> Event {
    Event::Trade(Trade {
        timestamp,
        price,
        size: 1.0,
    })
}

fn quote(timestamp: i64, mid: f64) -> Event {
    Event::Quote(Quote {
        timestamp,
        bid_price: mid - 1.0,
        bid_size: 1.0,
        ask_price: mid + 1.0,
        ask_size: 1.0,
    })
}

fn run<C: ModularCandle<Event> + Default + Clone>(events: &[Event]) -> Vec<C> {
    let rule = AlignedTimeRule::new(MillisecondPeriod::from_non_zero(10));
    let mut a =
        GenericAggregator::<C, _, Event>::new(rule, false).with_gaps(Gaps::Flat { max_candles: 5 });
    let mut bars = Vec::new();
    for e in events {
        trade_aggregation::Aggregator::update(&mut a, e, &mut bars);
    }
    bars.into_iter().map(|b| b.candle().clone()).collect()
}

/// Two candles with mid prices, then two empty periods, [10, 20) and [20, 30).
fn events_with_a_gap() -> [Event; 7] {
    [
        quote(0, 100.0),
        trade(1, 99.0),
        quote(2, 101.0),
        trade(3, 102.0),
        quote(35, 99.0),
        trade(36, 98.0),
        quote(45, 99.0),
    ]
}

#[test]
fn empty_candles_have_zero_sums() {
    let events = events_with_a_gap();
    let records: Vec<SumsRecord> = run::<Sums>(&events).iter().map(Sums::record).collect();
    assert_eq!(records.len(), 4);
    let r = (101.0_f64 / 100.0).ln();
    assert_eq!(records[0].n, 1);
    assert_eq!(records[0].up, 1);
    assert_eq!(records[0].max_abs, Some(r));
    assert!((records[0].rs_up - r * r).abs() < 1e-15);
    for empty in &records[1..3] {
        assert_eq!(empty.n, 0);
        assert_eq!(empty.up + empty.down + empty.flat, 0);
        assert_eq!(empty.rv, 0.0);
        assert_eq!(empty.tv, 0.0);
        assert_eq!(empty.cubic, 0.0);
        assert_eq!(empty.max_abs, None);
    }
    // The first return after the gap starts at the last mid before it.
    let down = (99.0_f64 / 101.0).ln();
    assert_eq!(records[3].n, 1);
    assert_eq!(records[3].down, 1);
    assert!((records[3].rs_down - down * down).abs() < 1e-15);
    assert!((records[3].quartic - down.powi(4)).abs() < 1e-18);
    // The last candle is still open: its unchanged tick is not in a record.
    assert!(records.iter().all(|r| r.flat == 0));
}

#[test]
fn empty_candles_have_no_path() {
    let records: Vec<PathRecord> = run::<Path>(&events_with_a_gap())
        .iter()
        .map(Path::record)
        .collect();
    assert_eq!(records.len(), 4);
    // Mid 100 at 0 and 101 at 2, closed by the time rule at 10.
    assert_eq!(records[0].efficiency, Some(1.0));
    for empty in &records[1..3] {
        assert_eq!(empty.efficiency, None);
        assert_eq!(empty.autocovariance, 0.0);
    }
    // The path after the gap starts at the last mid before it, 101, and falls to 99.
    assert_eq!(records[3].efficiency, Some(1.0));
    assert!(
        records[3].autocovariance < 0.0,
        "the fall reverts the rise before the gap"
    );
}
