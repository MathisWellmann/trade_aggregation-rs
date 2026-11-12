use std::marker::PhantomData;

use crate::{AggregationRule, Bar, BarKind, ModularCandle, Timestamped};

/// Defines the needed methods for any online `Aggregator`
pub trait Aggregator<Candle, E> {
    /// Updates the aggregation state with a new event.
    ///
    /// # Arguments:
    /// `event`: the event, e.g. a trade, to add to the aggregation process.
    ///     Events must arrive in time order.
    /// `out`: receives the candles that the event closed, in order.
    ///     An event can close more than one candle, e.g. after a gap in the data.
    fn update(&mut self, event: &E, out: &mut impl Extend<Bar<Candle>>);

    /// Get a reference to an unfinished `Candle`.
    /// Accessing a `Candle` using this method does not guarantee that the `AggregationRule` is respected.
    /// It is generally advised to call `update` instead and use the resulting `Candle` if its `Some`.
    fn unfinished_candle(&self) -> &Candle;
}

/// Which candle receives the trade that triggers the `AggregationRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TriggerTrade {
    /// The trade opens the next candle and is not part of the candle it closes.
    /// Every trade is counted once.
    /// The close of a candle can differ from the open of the next one,
    /// as with time candles on Tradingview.
    NextCandle,
    /// The trade is the last trade of the candle it closes.
    /// The next candle opens with the trade after it.
    /// Every trade is counted once and the close of a candle is the price of the triggering trade.
    ClosingCandle,
    /// The trade is the last trade of the candle it closes and the first trade of the next candle,
    /// so the close of a candle equals the open of the next one.
    /// Components that sum over trades (e.g. `Volume`, `NumTrades`) count this trade twice.
    Both,
}

/// What the aggregator emits for a period without events,
/// when the [deadline](AggregationRule::deadline) of a rule passes and the candle is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Gaps {
    /// Emit nothing: the next candle holds the next event.
    #[default]
    Skip,
    /// Emit an empty candle for each period, e.g. flat at the previous close,
    /// so that a window of n time candles always spans the same clock time.
    /// After `max_candles` empty candles in a row, skip the rest of the gap,
    /// so that an outage of the data does not flood the output.
    Flat {
        /// The most empty candles in a row.
        max_candles: u64,
    },
}

impl Gaps {
    /// The most empty candles in a row.
    fn max_candles(self) -> u64 {
        match self {
            Gaps::Skip => 0,
            Gaps::Flat { max_candles } => max_candles,
        }
    }
}

/// An `Aggregator` that is generic over
/// the type of Candle being produced,
/// as well as by which rule the candle is created
///
/// It closes a candle when the rule triggers on an event, or when the
/// [deadline](AggregationRule::deadline) of the rule passes.
/// A deadline passes when an event or a watermark ([`Self::advance_to`]) reaches it,
/// so a live consumer closes time candles on a heartbeat instead of waiting for the next event.
/// The candles do not depend on when the heartbeats come.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericAggregator<C, R, E> {
    candle: C,
    aggregation_rule: R,
    trigger_trade: TriggerTrade,
    gaps: Gaps,
    /// The number of empty candles emitted since the last event.
    empty_candles: u64,
    /// The number of candles closed so far, which is the `seq` of the next one.
    next_seq: u64,
    /// When the current candle opened, `None` before the first event.
    open_ts: Option<i64>,
    /// The number of events in the current candle.
    num_events: u64,
    /// The latest time the aggregator has seen, to check that time does not run backwards.
    now: Option<i64>,
    _event_type: PhantomData<E>,
}

impl<C, R, E> GenericAggregator<C, R, E>
where
    C: ModularCandle<E>,
    R: AggregationRule<E>,
    E: Timestamped,
{
    /// Create a new instance with a concrete aggregation rule
    /// and an empty candle.
    ///
    /// # Arguments:
    /// `aggregation_rule`: The rule that dictates when to trigger the creation of a new candle.
    /// `include_trade_that_triggered_rule`: If true, the trade that triggered a rule is included in the current candle
    ///     as well as the next one.
    ///     During some aggregations, the desired behaviour is that the trade that crosses the trigger boundary
    ///     is included in both the current and next candle.
    ///     Examples uses include ensuring the close and open price of the current and next candle are equal.
    ///     If that's desired, set the field to true during construction of `Self`.
    ///     E.g on Tradingview the time aggregation would have this set to `false`, which may create gaps between close and open of subsequent candles.
    ///     With `true`, components that sum over trades (e.g. `Volume`, `NumTrades`) count the triggering trade twice.
    ///     `false` is [`TriggerTrade::NextCandle`] and `true` is [`TriggerTrade::Both`].
    ///     Use [`Self::with_trigger_trade`] for [`TriggerTrade::ClosingCandle`], which counts every trade once.
    pub fn new(aggregation_rule: R, include_trade_that_triggered_rule: bool) -> Self {
        let trigger_trade = if include_trade_that_triggered_rule {
            TriggerTrade::Both
        } else {
            TriggerTrade::NextCandle
        };
        Self::with_trigger_trade(aggregation_rule, trigger_trade)
    }

    /// Create a new instance with a concrete aggregation rule
    /// and an empty candle.
    ///
    /// # Arguments:
    /// `aggregation_rule`: The rule that dictates when to trigger the creation of a new candle.
    /// `trigger_trade`: Which candle receives the trade that triggers the rule.
    pub fn with_trigger_trade(aggregation_rule: R, trigger_trade: TriggerTrade) -> Self {
        Self {
            candle: Default::default(),
            aggregation_rule,
            trigger_trade,
            gaps: Gaps::Skip,
            empty_candles: 0,
            next_seq: 0,
            open_ts: None,
            num_events: 0,
            now: None,
            _event_type: PhantomData,
        }
    }

    /// Set what to emit for periods without events, [`Gaps::Skip`] by default.
    pub fn with_gaps(self, gaps: Gaps) -> Self {
        Self { gaps, ..self }
    }

    /// Close every candle whose [deadline](AggregationRule::deadline) is at or before `watermark`,
    /// e.g. on a heartbeat, when no event with a later timestamp can arrive anymore.
    ///
    /// # Arguments:
    /// `watermark`: The time up to which all events have been passed to [`Aggregator::update`].
    /// `out`: Receives the closed candles, in order.
    ///
    /// # Panics:
    /// If `watermark` is earlier than an event or watermark seen before,
    /// or if the rule does not move its deadline forward.
    pub fn advance_to(&mut self, watermark: i64, out: &mut impl Extend<Bar<C>>) {
        self.assert_in_order(watermark);
        self.now = Some(watermark);
        if self.open_ts.is_none() {
            // No candle before the first event.
            return;
        }
        while let Some(deadline) = self.aggregation_rule.deadline() {
            if watermark < deadline {
                break;
            }
            if self.num_events == 0 && self.empty_candles >= self.gaps.max_candles() {
                self.aggregation_rule.skip_to(watermark);
                let next = self.aggregation_rule.deadline();
                assert!(
                    next.is_none_or(|next| next > watermark),
                    "skip_to({watermark}) must move the deadline past it, got {next:?}"
                );
                break;
            }
            if self.num_events == 0 {
                self.empty_candles += 1;
            }
            out.extend([self.close(deadline, BarKind::Deadline)]);
            self.aggregation_rule.on_deadline();
            let next = self.aggregation_rule.deadline();
            assert!(
                next.is_none_or(|next| next > deadline),
                "on_deadline must move the deadline {deadline} forward, got {next:?}"
            );
        }
    }

    /// The rule, e.g. to read the threshold of an [`Adaptive`](crate::Adaptive) rule.
    pub fn rule(&self) -> &R {
        &self.aggregation_rule
    }

    /// The `seq` the current candle gets when it closes.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// A snapshot of the unfinished candle as of `now`, e.g. for features over a window that ends now.
    /// `None` before the first event.
    ///
    /// # Panics:
    /// If `now` is earlier than the latest event,
    /// or if a deadline at or before `now` has not been processed with [`Self::advance_to`],
    /// as the unfinished candle would then hold a closed period.
    pub fn provisional(&self, now: i64) -> Option<Bar<C>> {
        let open_ts = self.open_ts?;
        self.assert_in_order(now);
        let deadline = self.aggregation_rule.deadline();
        assert!(
            deadline.is_none_or(|deadline| now < deadline),
            "call advance_to({now}) before provisional({now}), the deadline {deadline:?} passed"
        );
        let mut candle = self.candle.clone();
        candle.close_at(now);
        Some(Bar::new(
            candle,
            self.next_seq,
            (open_ts, now),
            self.num_events,
            BarKind::Provisional,
        ))
    }

    /// Time must not run backwards, or candles would close in the past.
    fn assert_in_order(&self, timestamp: i64) {
        if let Some(now) = self.now {
            assert!(
                timestamp >= now,
                "events must arrive in time order: {timestamp} after {now}"
            );
        }
    }

    /// Add `event` to the current candle.
    fn add(&mut self, event: &E) {
        self.candle.update(event);
        self.num_events += 1;
        self.empty_candles = 0;
    }

    /// Close the current candle at `close_ts` and open the next one there.
    fn close(&mut self, close_ts: i64, kind: BarKind) -> Bar<C> {
        let open_ts = self.open_ts.expect("a candle closes only after it opened");
        self.candle.close_at(close_ts);
        let bar = Bar::new(
            self.candle.clone(),
            self.next_seq,
            (open_ts, close_ts),
            self.num_events,
            kind,
        );
        self.candle.reset();
        self.next_seq += 1;
        self.open_ts = Some(close_ts);
        self.num_events = 0;
        bar
    }
}

impl<C, R, E> Aggregator<C, E> for GenericAggregator<C, R, E>
where
    C: ModularCandle<E>,
    R: AggregationRule<E>,
    E: Timestamped,
{
    /// Closes the candles whose deadline passed before `event` (see [`GenericAggregator::advance_to`]),
    /// then adds `event`: an event at or after the deadline never goes into the period that ended,
    /// whatever the [`TriggerTrade`].
    fn update(&mut self, event: &E, out: &mut impl Extend<Bar<C>>) {
        let timestamp = event.timestamp();
        self.advance_to(timestamp, out);
        self.open_ts.get_or_insert(timestamp);

        if !self.aggregation_rule.should_trigger(event) {
            self.add(event);
            return;
        }
        if matches!(
            self.trigger_trade,
            TriggerTrade::ClosingCandle | TriggerTrade::Both
        ) {
            self.add(event);
        }
        out.extend([self.close(timestamp, BarKind::Rule)]);
        if matches!(
            self.trigger_trade,
            TriggerTrade::NextCandle | TriggerTrade::Both
        ) {
            self.add(event);
        }
    }

    fn unfinished_candle(&self) -> &C {
        &self.candle
    }
}

#[cfg(test)]
mod tests {
    use trade_aggregation_derive::Candle;

    use super::*;
    use crate::{
        aggregation_rules::test_utils::closed_periods,
        candle_components::{Close, NumTrades, Open, Volume},
        load_trades_from_csv, TickRule, TimeRule, Trade, M1,
    };

    #[derive(Default, Debug, Clone, Candle)]
    struct CountingCandle {
        open: Open,
        close: Close,
        volume: Volume,
        num_trades: NumTrades<u32>,
    }

    /// Ten trades with prices 1..=10 and sizes 1..=10, alternating sides.
    fn ten_trades() -> Vec<Trade> {
        (1..=10)
            .map(|i| Trade {
                timestamp: i,
                price: i as f64,
                size: if i % 2 == 0 { i as f64 } else { -(i as f64) },
            })
            .collect()
    }

    /// Aggregate `ten_trades` into candles of three trades.
    fn aggregate_ten_bars(
        trigger_trade: TriggerTrade,
    ) -> (
        Vec<Bar<CountingCandle>>,
        GenericAggregator<CountingCandle, TickRule, Trade>,
    ) {
        let mut a = GenericAggregator::<CountingCandle, TickRule, Trade>::with_trigger_trade(
            TickRule::new(3),
            trigger_trade,
        );
        let mut bars = Vec::new();
        for t in &ten_trades() {
            a.update(t, &mut bars);
        }
        (bars, a)
    }

    /// The finished candles of `aggregate_ten_bars` and the unfinished one.
    fn aggregate_ten(trigger_trade: TriggerTrade) -> (Vec<CountingCandle>, CountingCandle) {
        let (bars, a) = aggregate_ten_bars(trigger_trade);
        (
            bars.into_iter().map(Bar::into_candle).collect(),
            a.unfinished_candle().clone(),
        )
    }

    fn total_volume(candles: &[CountingCandle], unfinished: &CountingCandle) -> f64 {
        candles.iter().map(|c| c.volume()).sum::<f64>() + unfinished.volume()
    }

    fn total_trades(candles: &[CountingCandle], unfinished: &CountingCandle) -> u32 {
        candles.iter().map(|c| c.num_trades()).sum::<u32>() + unfinished.num_trades()
    }

    #[test]
    fn trigger_trade_next_candle_counts_every_trade_once() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::NextCandle);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 10);
        assert_eq!(total_volume(&candles, &unfinished), 55.0);
        // The trigger trades 3, 6 and 9 open the next candle.
        assert_eq!(candles[0].num_trades(), 2);
        assert_eq!(candles[0].close(), 2.0);
        assert_eq!(candles[1].open(), 3.0);
        assert_eq!(candles[1].close(), 5.0);
        assert_eq!(unfinished.open(), 9.0);
    }

    #[test]
    fn trigger_trade_closing_candle_counts_every_trade_once() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::ClosingCandle);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 10);
        assert_eq!(total_volume(&candles, &unfinished), 55.0);
        // The trigger trades 3, 6 and 9 close their candle.
        for (candle, close) in candles.iter().zip([3.0, 6.0, 9.0]) {
            assert_eq!(candle.num_trades(), 3);
            assert_eq!(candle.close(), close);
        }
        assert_eq!(candles[1].open(), 4.0);
        assert_eq!(unfinished.num_trades(), 1);
        assert_eq!(unfinished.open(), 10.0);
    }

    #[test]
    fn trigger_trade_both_counts_the_trigger_trade_twice() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::Both);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 13);
        assert_eq!(total_volume(&candles, &unfinished), 55.0 + 3.0 + 6.0 + 9.0);
        for pair in candles.windows(2) {
            assert_eq!(pair[0].close(), pair[1].open());
        }
        assert_eq!(candles[2].close(), unfinished.open());
    }

    #[test]
    fn new_maps_the_bool_to_the_documented_trigger_trade() {
        let a = GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), false);
        assert_eq!(a.trigger_trade, TriggerTrade::NextCandle);
        let a = GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), true);
        assert_eq!(a.trigger_trade, TriggerTrade::Both);
    }

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
        close: Close,
        num_trades: NumTrades<u32>,
    }

    #[test]
    fn generic_aggregator() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
            .expect("Could not load trades from file!");

        let rule = TimeRule::new(M1);
        let mut a = GenericAggregator::<MyCandle, TimeRule, Trade>::new(rule, false);

        let mut bars = Vec::new();
        for t in trades.iter() {
            a.update(t, &mut bars);
        }
        let timestamps: Vec<i64> = trades.iter().map(|t| t.timestamp).collect();
        assert_eq!(
            bars.len(),
            closed_periods(&timestamps, timestamps[0], M1.get() as i64)
        );
    }

    /// Trades with gaps of up to a few seconds, from a seeded xorshift generator.
    fn gappy_trades(n: usize, seed: u64) -> Vec<Trade> {
        let mut state = seed;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut timestamp = 0;
        (0..n)
            .map(|_| {
                let gaps = [0, 1, 5, 40, 300, 2_500];
                timestamp += gaps[(next() % 6) as usize];
                let price = 100.0 + (next() % 50) as f64;
                let size = if next() % 2 == 0 { 1.0 } else { -2.0 };
                Trade {
                    timestamp,
                    price,
                    size,
                }
            })
            .collect()
    }

    /// The closed bars of `trades`, with a heartbeat (watermark) before every trade
    /// for which `heartbeat` returns a time, and a final watermark.
    fn run_with_heartbeats<R>(
        rule: R,
        gaps: Gaps,
        trades: &[Trade],
        heartbeat: impl Fn(usize, &Trade) -> Option<i64>,
    ) -> Vec<Bar<CountingCandle>>
    where
        R: AggregationRule<Trade>,
    {
        let mut a = GenericAggregator::<CountingCandle, R, Trade>::new(rule, false).with_gaps(gaps);
        let mut bars = Vec::new();
        for (i, t) in trades.iter().enumerate() {
            if let Some(watermark) = heartbeat(i, t) {
                a.advance_to(watermark, &mut bars);
            }
            a.update(t, &mut bars);
        }
        let end = trades.last().expect("trades").timestamp + 10_000;
        a.advance_to(end, &mut bars);
        bars
    }

    /// The bars as comparable tuples.
    fn summary(bars: &[Bar<CountingCandle>]) -> Vec<(u64, i64, i64, u64, BarKind, String)> {
        bars.iter()
            .map(|b| {
                (
                    b.seq(),
                    b.open_ts(),
                    b.close_ts(),
                    b.num_events(),
                    b.kind(),
                    format!("{:?}", b.candle()),
                )
            })
            .collect()
    }

    fn assert_heartbeats_do_not_change_the_bars<R: AggregationRule<Trade> + Clone>(rule: R) {
        let trades = gappy_trades(3_000, 7);
        for gaps in [Gaps::Skip, Gaps::Flat { max_candles: 4 }] {
            let replay = run_with_heartbeats(rule.clone(), gaps, &trades, |_, _| None);
            assert!(replay.len() > 10, "the test needs candles");
            for step in [1, 3, 17] {
                // A heartbeat just before the trade, at a time no later trade can precede.
                let live = run_with_heartbeats(rule.clone(), gaps, &trades, |i, t| {
                    (i % step == 0).then_some(t.timestamp)
                });
                assert_eq!(summary(&replay), summary(&live), "{gaps:?}, every {step}");
                // A heartbeat halfway between two trades.
                let live = run_with_heartbeats(rule.clone(), gaps, &trades, |i, t| {
                    (i > 0 && i % step == 0).then(|| (trades[i - 1].timestamp + t.timestamp) / 2)
                });
                assert_eq!(
                    summary(&replay),
                    summary(&live),
                    "{gaps:?}, half, every {step}"
                );
            }
        }
    }

    #[test]
    fn heartbeats_do_not_change_time_candles() {
        assert_heartbeats_do_not_change_the_bars(crate::AlignedTimeRule::new(
            crate::MillisecondPeriod::from_non_zero(1_000),
        ));
    }

    #[test]
    fn heartbeats_do_not_change_capped_tick_candles() {
        assert_heartbeats_do_not_change_the_bars(crate::MaxDuration::new(
            TickRule::new(20),
            crate::MillisecondPeriod::from_non_zero(700),
        ));
        assert_heartbeats_do_not_change_the_bars(crate::AnyRule::new(
            TickRule::new(20),
            crate::AlignedTimeRule::new(crate::MillisecondPeriod::from_non_zero(5_000)),
        ));
    }

    #[test]
    fn no_bar_closes_after_the_event_or_watermark_that_closed_it() {
        let trades = gappy_trades(2_000, 11);
        let mut a = GenericAggregator::<CountingCandle, _, Trade>::new(
            crate::AlignedTimeRule::new(crate::MillisecondPeriod::from_non_zero(1_000)),
            false,
        )
        .with_gaps(Gaps::Flat { max_candles: 2 });
        let mut bars = Vec::new();
        for t in &trades {
            let before = bars.len();
            a.update(t, &mut bars);
            for bar in &bars[before..] {
                assert!(bar.close_ts() <= t.timestamp);
                assert!(bar.open_ts() <= bar.close_ts());
            }
        }
        let seqs: Vec<u64> = bars.iter().map(Bar::seq).collect();
        assert_eq!(seqs, (0..bars.len() as u64).collect::<Vec<_>>());
    }

    #[test]
    #[should_panic(expected = "call advance_to")]
    fn provisional_needs_the_passed_deadlines_closed() {
        let mut a = GenericAggregator::<CountingCandle, _, Trade>::new(
            crate::AlignedTimeRule::new(crate::MillisecondPeriod::from_non_zero(100)),
            false,
        );
        let mut bars = Vec::new();
        a.update(&ten_trades()[0], &mut bars);
        let _ = a.provisional(150);
    }

    #[test]
    fn bars_number_and_stamp_the_candles() {
        let (bars, a) = aggregate_ten_bars(TriggerTrade::NextCandle);
        let seqs: Vec<u64> = bars.iter().map(Bar::seq).collect();
        assert_eq!(seqs, vec![0, 1, 2]);
        // The trigger trades 3, 6 and 9 close the candles at their timestamps.
        let stamps: Vec<(i64, i64)> = bars.iter().map(|b| (b.open_ts(), b.close_ts())).collect();
        assert_eq!(stamps, vec![(1, 3), (3, 6), (6, 9)]);
        let events: Vec<u64> = bars.iter().map(Bar::num_events).collect();
        assert_eq!(events, vec![2, 3, 3]);
        assert!(bars.iter().all(|b| b.kind() == BarKind::Rule));
        assert_eq!(a.next_seq(), 3);
    }

    #[test]
    fn closing_candle_counts_the_trigger_in_the_closed_bar() {
        let (bars, _) = aggregate_ten_bars(TriggerTrade::ClosingCandle);
        let events: Vec<u64> = bars.iter().map(Bar::num_events).collect();
        assert_eq!(events, vec![3, 3, 3]);
        let both: Vec<u64> = aggregate_ten_bars(TriggerTrade::Both)
            .0
            .iter()
            .map(Bar::num_events)
            .collect();
        assert_eq!(both, vec![3, 4, 4]);
    }

    #[test]
    fn provisional_snapshots_the_unfinished_candle() {
        let mut a =
            GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), false);
        assert!(
            a.provisional(0).is_none(),
            "no candle before the first event"
        );
        let mut bars = Vec::new();
        for t in &ten_trades()[..4] {
            a.update(t, &mut bars);
        }
        assert_eq!(bars.len(), 1);
        let p = a.provisional(7).expect("a candle is open");
        assert_eq!(p.kind(), BarKind::Provisional);
        assert_eq!(p.seq(), 1, "the seq the candle gets when it closes");
        assert_eq!((p.open_ts(), p.close_ts()), (3, 7));
        assert_eq!(p.num_events(), 2);
        assert_eq!(p.candle().open(), 3.0);
        assert_eq!(p.candle().close(), 4.0);
    }

    #[test]
    #[should_panic(expected = "time order")]
    fn events_must_arrive_in_time_order() {
        let mut a =
            GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), false);
        let mut bars = Vec::new();
        let trades = ten_trades();
        a.update(&trades[1], &mut bars);
        a.update(&trades[0], &mut bars);
    }

    #[test]
    #[should_panic(expected = "time order")]
    fn a_provisional_bar_cannot_be_in_the_past() {
        let mut a =
            GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), false);
        let mut bars = Vec::new();
        a.update(&ten_trades()[1], &mut bars);
        let _ = a.provisional(1);
    }

    #[test]
    fn candle_macro() {
        let my_candle = MyCandle::default();
        println!("my_candle: {:?}", my_candle);

        // make sure the 'open' and 'close' getters have been generated
        println!("open: {}", my_candle.open());
        println!("close: {}", my_candle.close());
    }

    /// The candle of the checkpoint test, which serde can save.
    #[cfg(feature = "serde")]
    #[derive(Default, Debug, Clone, Candle, serde::Serialize, serde::Deserialize)]
    struct SavedCandle {
        open: Open,
        close: Close,
        volume: Volume,
        num_trades: NumTrades<u32>,
    }

    /// Aggregate `trades`, saving the aggregator to JSON and restoring it before each trade
    /// for which `restore` is true.
    #[cfg(feature = "serde")]
    fn run_with_restarts<R>(
        rule: R,
        trades: &[Trade],
        restore: impl Fn(usize) -> bool,
    ) -> Vec<Bar<SavedCandle>>
    where
        R: AggregationRule<Trade> + serde::Serialize + serde::de::DeserializeOwned,
    {
        let mut a = GenericAggregator::<SavedCandle, R, Trade>::new(rule, false)
            .with_gaps(Gaps::Flat { max_candles: 3 });
        let mut bars = Vec::new();
        for (i, t) in trades.iter().enumerate() {
            if restore(i) {
                let saved = serde_json::to_string(&a).expect("serializable");
                a = serde_json::from_str(&saved).expect("deserializable");
            }
            a.update(t, &mut bars);
        }
        bars
    }

    #[cfg(feature = "serde")]
    fn assert_restarts_do_not_change_the_bars<R>(rule: R)
    where
        R: AggregationRule<Trade> + Clone + serde::Serialize + serde::de::DeserializeOwned,
    {
        let trades = gappy_trades(3_000, 5);
        let uninterrupted = run_with_restarts(rule.clone(), &trades, |_| false);
        assert!(uninterrupted.len() > 10, "the test needs candles");
        let as_json = |bars: &[Bar<SavedCandle>]| serde_json::to_string(bars).expect("bars");
        for step in [1, 7, 101] {
            let restarted = run_with_restarts(rule.clone(), &trades, |i| i % step == 0);
            assert_eq!(as_json(&uninterrupted), as_json(&restarted), "every {step}");
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_restored_aggregator_continues_where_it_was_saved() {
        use crate::{
            Adaptive, AlignedTimeRule, AnyRule, By, MaxDuration, MillisecondPeriod, Scaling,
            SignedVolumeRule,
        };
        assert_restarts_do_not_change_the_bars(AlignedTimeRule::new(
            MillisecondPeriod::from_non_zero(1_000),
        ));
        let delta = Adaptive::new(
            SignedVolumeRule::new(4.0, By::Quote).expect("valid threshold"),
            MillisecondPeriod::from_non_zero(2_000),
            0.1,
            0.04,
            400.0,
        )
        .expect("valid parameters")
        .with_scaling(Scaling::Diffusive);
        assert_restarts_do_not_change_the_bars(AnyRule::new(
            MaxDuration::new(delta, MillisecondPeriod::from_non_zero(10_000)),
            AlignedTimeRule::new(MillisecondPeriod::from_non_zero(60_000)),
        ));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_zero_period_does_not_deserialize() {
        let saved = serde_json::to_string(&TimeRule::new(M1)).expect("serializable");
        assert!(saved.contains("60000"), "{saved}");
        let broken = saved.replace("60000", "0");
        assert!(serde_json::from_str::<TimeRule>(&broken).is_err());
    }
}
