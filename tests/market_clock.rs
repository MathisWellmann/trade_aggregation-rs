//! A market clock: one rule fires on a leader symbol, and the resulting instants force-close
//! the aggregator of every symbol in a cross-section, so a downstream model gets one row per
//! symbol per clock tick instead of each symbol closing candles on its own asynchronous
//! schedule.

use trade_aggregation::{
    candle_components::Close, AggregationRule, Aggregator, BarKind, Candle, GenericAggregator,
    TickRule, Trade,
};

#[derive(Debug, Default, Clone, Candle)]
struct SymbolCandle {
    close: Close,
}

fn trade(timestamp: i64, price: f64, size: f64) -> Trade {
    Trade {
        timestamp,
        price,
        size,
    }
}

#[test]
fn a_market_clock_force_closes_two_symbols_at_the_same_instants() {
    // Symbol A trades every tick; symbol B trades at other, sparser instants.
    // Neither aggregator's own rule ever triggers in this short a stream (1000 ticks),
    // so every closed bar below comes from the market clock, not from the symbol's own rule.
    let a_trades = [
        trade(1, 100.0, 1.0),
        trade(2, 101.0, 1.0),
        trade(3, 100.5, 1.0),
        trade(4, 102.0, 1.0),
        trade(5, 101.5, 1.0),
        trade(6, 103.0, 1.0),
        trade(7, 103.5, 1.0),
    ];
    let b_trades = [
        trade(2, 50.0, 2.0),
        trade(5, 51.0, 2.0),
        trade(7, 49.0, 2.0),
    ];

    // The market clock: a coordinator rule on the leader symbol A, ticking every 3 trades.
    // It is not part of either aggregator; its only job is to decide when to force-close.
    let mut leader = TickRule::new(3);
    let mut a =
        GenericAggregator::<SymbolCandle, TickRule, Trade>::new(TickRule::new(1_000), false);
    let mut b =
        GenericAggregator::<SymbolCandle, TickRule, Trade>::new(TickRule::new(1_000), false);
    let (mut a_bars, mut b_bars) = (Vec::new(), Vec::new());
    let mut b_trades = b_trades.iter().peekable();

    for t in &a_trades {
        // Symbol B is driven by its own, sparser stream, merged in time order.
        while b_trades
            .peek()
            .is_some_and(|bt| bt.timestamp <= t.timestamp)
        {
            let bt = b_trades.next().expect("peeked");
            b.update(bt, &mut b_bars);
        }
        a.update(t, &mut a_bars);
        if leader.should_trigger(t) {
            // The clock ticked: sample every symbol at the same instant, whatever each
            // symbol's own rule says.
            a.force_close(t.timestamp, &mut a_bars);
            b.force_close(t.timestamp, &mut b_bars);
        }
    }

    let forced_close_ts = |bars: &[trade_aggregation::Bar<SymbolCandle>]| -> Vec<i64> {
        bars.iter()
            .filter(|bar| bar.kind() == BarKind::Forced)
            .map(|bar| bar.close_ts())
            .collect()
    };
    // Both symbols close their forced candles at the same instants: a cross-sectional model
    // can join the two series row by row on `close_ts` without lookahead.
    assert_eq!(forced_close_ts(&a_bars), vec![3, 6]);
    assert_eq!(forced_close_ts(&b_bars), vec![3, 6]);

    // What each candle holds still reflects each symbol's own, asynchronous trade flow.
    let num_events = |bars: &[trade_aggregation::Bar<SymbolCandle>]| -> Vec<u64> {
        bars.iter()
            .filter(|bar| bar.kind() == BarKind::Forced)
            .map(|bar| bar.num_events())
            .collect()
    };
    assert_eq!(num_events(&a_bars), vec![3, 3], "A trades on every tick");
    assert_eq!(num_events(&b_bars), vec![1, 1], "B trades more rarely");
    assert!(a_bars[0].candle().close() > 0.0);
}
