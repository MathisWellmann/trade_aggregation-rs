//! Rules built from other rules: either of two rules, both of two rules,
//! and lower and upper bounds on the duration of a candle.
//!
//! The duration of a candle is measured from the close of the previous candle
//! (the trade that triggered it, or its deadline), or from the first trade, to the current trade.
//! With [`TriggerTrade::ClosingCandle`](crate::TriggerTrade::ClosingCandle) the candle opens one trade later,
//! so the measured duration is slightly longer than the candle.
//!
//! The combinators forward the [deadlines](AggregationRule::deadline) of their inner rules,
//! so that e.g. `AnyRule<R, AlignedTimeRule>` also closes a candle at each period end
//! without a trade.

use crate::{AggregationRule, MillisecondPeriod, Timestamped};

/// The earlier of two optional deadlines.
fn earliest(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, None) => a,
        (None, b) => b,
    }
}

/// Close the candle of an inner rule at a deadline: the rule whose deadline it is moves on,
/// any other rule forgets the candle.
fn close_at_deadline<E, R>(rule: &mut R, deadline: i64)
where
    R: AggregationRule<E>,
{
    if rule.deadline() == Some(deadline) {
        rule.on_deadline();
    } else {
        rule.reset();
    }
}

/// Reset the inner rules that did not trigger on the current trade,
/// because a candle closed while they were still accumulating.
/// A rule that did trigger has already restarted its own state.
fn reset_untriggered<E, R>(rule: &mut R, triggered_now: bool)
where
    R: AggregationRule<E>,
{
    if !triggered_now {
        rule.reset();
    }
}

/// The time from `open_timestamp` to the timestamp of `trade`, in the units of `trade`.
fn elapsed<E: Timestamped>(open_timestamp: i64, trade: &E) -> i64 {
    let elapsed = trade.timestamp().saturating_sub(open_timestamp);
    debug_assert!(elapsed >= 0, "trades must arrive in time order");
    elapsed
}

/// Which inner rule of an [`AnyRule`] or an [`AllRule`] is responsible for the most recent close.
///
/// For [`AnyRule`], the rule that triggered first (or, at a deadline, the rule whose deadline it
/// was). For [`AllRule`], the rule that completed the pair last (or, at a deadline, the rule
/// whose deadline let the pending one through). Which criterion closed a hybrid bar is itself a
/// feature of the bar (e.g. volume vs. volatility), and the share of candles closed by a timeout
/// rather than the rule is a calibration diagnostic for the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Fired {
    /// Rule `a` alone.
    First,
    /// Rule `b` alone.
    Second,
    /// Both rules, on the same trade or at the same deadline.
    Both,
}

impl Fired {
    /// Which rule(s) a close belongs to, from whether each one fired on it.
    ///
    /// # Panics (debug only):
    /// If neither fired: a close always has a reason.
    fn from_triggers(a: bool, b: bool) -> Self {
        debug_assert!(a || b, "a close needs at least one rule to have fired");
        match (a, b) {
            (true, true) => Fired::Both,
            (true, false) => Fired::First,
            (false, _) => Fired::Second,
        }
    }
}

/// Closes the candle as soon as either rule triggers,
/// e.g. a dollar volume rule or a volatility rule, whichever comes first ("hybrid bars").
///
/// Both rules see every trade.
/// When one rule closes the candle, the other one is [reset](AggregationRule::reset),
/// so that both start the next candle from scratch.
/// Nest `AnyRule`s to combine more than two rules.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnyRule<A, B> {
    a: A,
    b: B,
    /// Which rule closed the most recent candle, `None` before the first close.
    /// Not touched by [`AggregationRule::reset`]: it is a record of the last close,
    /// not per-candle accumulator state.
    last_trigger: Option<Fired>,
}

impl<A, B> AnyRule<A, B> {
    /// Combine two rules, of which either one closes the candle.
    pub fn new(a: A, b: B) -> Self {
        Self {
            a,
            b,
            last_trigger: None,
        }
    }

    /// Which rule closed the most recent candle that this rule closed,
    /// `None` before the first close.
    pub fn last_trigger(&self) -> Option<Fired> {
        self.last_trigger
    }

    /// The first rule, e.g. to read its state after a candle it closed,
    /// such as the last event of an `IntrinsicEventRule`.
    pub fn first(&self) -> &A {
        &self.a
    }

    /// The second rule.
    pub fn second(&self) -> &B {
        &self.b
    }
}

impl<E, A, B> AggregationRule<E> for AnyRule<A, B>
where
    A: AggregationRule<E>,
    B: AggregationRule<E>,
{
    fn should_trigger(&mut self, trade: &E) -> bool {
        // Evaluate both rules, so that neither misses a trade.
        let a = self.a.should_trigger(trade);
        let b = self.b.should_trigger(trade);
        let trigger = a || b;
        if trigger {
            reset_untriggered::<E, _>(&mut self.a, a);
            reset_untriggered::<E, _>(&mut self.b, b);
            self.last_trigger = Some(Fired::from_triggers(a, b));
        }
        trigger
    }

    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }

    /// The earlier deadline of the two rules.
    fn deadline(&self) -> Option<i64> {
        earliest(self.a.deadline(), self.b.deadline())
    }

    fn on_deadline(&mut self) {
        let deadline = AggregationRule::<E>::deadline(self)
            .expect("the aggregator closes a candle at a deadline only if there is one");
        let a_at_deadline = self.a.deadline() == Some(deadline);
        let b_at_deadline = self.b.deadline() == Some(deadline);
        self.last_trigger = Some(Fired::from_triggers(a_at_deadline, b_at_deadline));
        close_at_deadline::<E, _>(&mut self.a, deadline);
        close_at_deadline::<E, _>(&mut self.b, deadline);
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.a.skip_to(timestamp);
        self.b.skip_to(timestamp);
    }
}

/// Closes the candle once both rules have triggered since the candle opened,
/// e.g. at least 1000 trades and at least 50 units of volume.
///
/// A rule that triggers first is remembered until the other one triggers too.
/// When the candle closes, a rule that did not trigger on that trade is
/// [reset](AggregationRule::reset), so that both start the next candle from scratch.
/// Nest `AllRule`s to combine more than two rules.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AllRule<A, B> {
    a: A,
    b: B,
    a_triggered: bool,
    b_triggered: bool,
    /// Which rule completed the pair last, `None` before the first close.
    /// Not touched by [`AggregationRule::reset`]: it is a record of the last close,
    /// not per-candle accumulator state.
    last_trigger: Option<Fired>,
}

impl<A, B> AllRule<A, B> {
    /// Combine two rules, which both have to trigger to close the candle.
    pub fn new(a: A, b: B) -> Self {
        Self {
            a,
            b,
            a_triggered: false,
            b_triggered: false,
            last_trigger: None,
        }
    }

    /// Which rule completed the pair last and so closed the most recent candle that this
    /// rule closed (the other rule had already triggered and was waiting), `None` before
    /// the first close.
    pub fn last_trigger(&self) -> Option<Fired> {
        self.last_trigger
    }

    /// The first rule, e.g. to read its state after a candle it closed,
    /// such as the last event of an `IntrinsicEventRule`.
    pub fn first(&self) -> &A {
        &self.a
    }

    /// The second rule.
    pub fn second(&self) -> &B {
        &self.b
    }
}

impl<E, A, B> AggregationRule<E> for AllRule<A, B>
where
    A: AggregationRule<E>,
    B: AggregationRule<E>,
{
    fn should_trigger(&mut self, trade: &E) -> bool {
        let a = self.a.should_trigger(trade);
        let b = self.b.should_trigger(trade);
        self.a_triggered |= a;
        self.b_triggered |= b;
        let trigger = self.a_triggered && self.b_triggered;
        if trigger {
            reset_untriggered::<E, _>(&mut self.a, a);
            reset_untriggered::<E, _>(&mut self.b, b);
            self.last_trigger = Some(Fired::from_triggers(a, b));
            self.a_triggered = false;
            self.b_triggered = false;
        }
        trigger
    }

    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
        self.a_triggered = false;
        self.b_triggered = false;
    }

    /// Once one rule has triggered, the deadline of the other one.
    /// Before, `None`: a deadline of one rule alone does not close the candle,
    /// and that rule triggers on the next event instead.
    fn deadline(&self) -> Option<i64> {
        match (self.a_triggered, self.b_triggered) {
            (true, false) => self.b.deadline(),
            (false, true) => self.a.deadline(),
            _ => None,
        }
    }

    fn on_deadline(&mut self) {
        let deadline = AggregationRule::<E>::deadline(self)
            .expect("the aggregator closes a candle at a deadline only if there is one");
        // `deadline()` is `Some` only when exactly one side is pending; the deadline lets
        // the other side (the one not yet triggered) through, which completes the pair.
        self.last_trigger = Some(if self.a_triggered {
            Fired::Second
        } else {
            Fired::First
        });
        close_at_deadline::<E, _>(&mut self.a, deadline);
        close_at_deadline::<E, _>(&mut self.b, deadline);
        self.a_triggered = false;
        self.b_triggered = false;
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.a.skip_to(timestamp);
        self.b.skip_to(timestamp);
    }
}

/// Keeps a candle open for at least `min_duration`,
/// so that a burst of activity does not produce a flood of tiny candles.
///
/// If the inner rule triggers earlier, the trigger is remembered and the candle closes
/// `min_duration` after it opened: at that [deadline](AggregationRule::deadline) in an aggregator,
/// or at the first trade from then on when the rule is used alone.
/// Further triggers of the inner rule in the meantime merge into that candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MinDuration<R> {
    inner: R,
    min_duration: MillisecondPeriod,
    open_timestamp: Option<i64>,
    pending: bool,
}

impl<R> MinDuration<R> {
    /// Wrap `inner`, keeping every candle open for at least `min_duration`.
    pub fn new(inner: R, min_duration: MillisecondPeriod) -> Self {
        Self {
            inner,
            min_duration,
            open_timestamp: None,
            pending: false,
        }
    }

    /// The inner rule, e.g. to read its state after a candle it closed.
    pub fn inner(&self) -> &R {
        &self.inner
    }
}

impl<E, R> AggregationRule<E> for MinDuration<R>
where
    E: Timestamped,
    R: AggregationRule<E>,
{
    fn should_trigger(&mut self, trade: &E) -> bool {
        let open_timestamp = *self.open_timestamp.get_or_insert(trade.timestamp());
        let inner = self.inner.should_trigger(trade);
        self.pending |= inner;
        let min_units = self.min_duration.in_units(E::TIMESTAMP_RESOLUTION);
        let trigger = self.pending && elapsed(open_timestamp, trade) >= min_units;
        if trigger {
            reset_untriggered::<E, _>(&mut self.inner, inner);
            self.pending = false;
            self.open_timestamp = Some(trade.timestamp());
        }
        trigger
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.open_timestamp = None;
        self.pending = false;
    }

    /// With a pending trigger, `min_duration` after the candle opened.
    /// Otherwise the deadline of the inner rule, but not before that.
    fn deadline(&self) -> Option<i64> {
        let min_units = self.min_duration.in_units(E::TIMESTAMP_RESOLUTION);
        let min_end = self
            .open_timestamp
            .map(|open| open.saturating_add(min_units));
        if self.pending {
            return min_end;
        }
        self.inner
            .deadline()
            .map(|deadline| min_end.map_or(deadline, |min_end| deadline.max(min_end)))
    }

    fn on_deadline(&mut self) {
        let deadline = AggregationRule::<E>::deadline(self)
            .expect("the aggregator closes a candle at a deadline only if there is one");
        if self.pending {
            // The inner rule triggered before and restarted then: forget what came since.
            self.inner.reset();
        } else {
            self.inner.on_deadline();
        }
        self.pending = false;
        self.open_timestamp = Some(deadline);
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.inner.skip_to(timestamp);
        self.open_timestamp = None;
        self.pending = false;
    }
}

/// Closes a candle `max_duration` after it opened, unless the inner rule closed it before,
/// so that quiet markets still produce candles.
///
/// In an aggregator the candle closes at that [deadline](AggregationRule::deadline) without a trade.
/// Used alone, the rule only sees trades and triggers at the first trade from then on.
/// On a timeout the inner rule is [reset](AggregationRule::reset).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxDuration<R> {
    inner: R,
    max_duration: MillisecondPeriod,
    open_timestamp: Option<i64>,
}

impl<R> MaxDuration<R> {
    /// Wrap `inner`, closing every candle after at most `max_duration`.
    pub fn new(inner: R, max_duration: MillisecondPeriod) -> Self {
        Self {
            inner,
            max_duration,
            open_timestamp: None,
        }
    }

    /// The inner rule, e.g. to read its state after a candle it closed.
    pub fn inner(&self) -> &R {
        &self.inner
    }
}

impl<E, R> AggregationRule<E> for MaxDuration<R>
where
    E: Timestamped,
    R: AggregationRule<E>,
{
    fn should_trigger(&mut self, trade: &E) -> bool {
        let open_timestamp = *self.open_timestamp.get_or_insert(trade.timestamp());
        let inner = self.inner.should_trigger(trade);
        let max_units = self.max_duration.in_units(E::TIMESTAMP_RESOLUTION);
        let timed_out = elapsed(open_timestamp, trade) >= max_units;
        let trigger = inner || timed_out;
        if trigger {
            reset_untriggered::<E, _>(&mut self.inner, inner);
            self.open_timestamp = Some(trade.timestamp());
        }
        trigger
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.open_timestamp = None;
    }

    /// `max_duration` after the candle opened, or the deadline of the inner rule if earlier.
    fn deadline(&self) -> Option<i64> {
        let max_units = self.max_duration.in_units(E::TIMESTAMP_RESOLUTION);
        let timeout = self
            .open_timestamp
            .map(|open| open.saturating_add(max_units));
        earliest(timeout, self.inner.deadline())
    }

    fn on_deadline(&mut self) {
        let deadline = AggregationRule::<E>::deadline(self)
            .expect("the aggregator closes a candle at a deadline only if there is one");
        close_at_deadline::<E, _>(&mut self.inner, deadline);
        self.open_timestamp = Some(deadline);
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.inner.skip_to(timestamp);
        self.open_timestamp = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, trigger_indices},
        By, ThresholdRule, TickRule, Trade, VolumeRule,
    };

    /// `n` trades one millisecond apart, each of size 1.
    fn trades(n: i64) -> Vec<Trade> {
        (0..n).map(|ts| trade(ts, 100.0, 1.0)).collect()
    }

    #[test]
    fn any_rule_closes_on_the_first_rule_and_resets_the_other() {
        let mut sizes = [1.0; 12];
        sizes[1] = 10.0;
        let trades: Vec<Trade> = sizes
            .iter()
            .zip(0..)
            .map(|(size, ts)| trade(ts, 100.0, *size))
            .collect();
        let mut rule = AnyRule::new(
            TickRule::new(5),
            VolumeRule::new(10.0, By::Quote).expect("valid threshold"),
        );
        // The volume rule fires on trade 1. The tick rule is reset there,
        // so it counts trades 2..=6 instead of firing on trade 4.
        assert_eq!(trigger_indices(&mut rule, &trades), vec![1, 6, 11]);
    }

    #[test]
    fn any_rule_reset_resets_both_rules() {
        let mut rule = AnyRule::new(TickRule::new(3), TickRule::new(4));
        let t = trades(6);
        assert!(trigger_indices(&mut rule, &t[..2]).is_empty());
        AggregationRule::<Trade>::reset(&mut rule);
        assert_eq!(trigger_indices(&mut rule, &t[2..]), vec![2]);
    }

    #[test]
    fn all_rule_waits_for_both_rules() {
        let mut rule = AllRule::new(TickRule::new(2), TickRule::new(5));
        // The 2-tick rule fires on 1 and 3, the 5-tick rule on 4: close on 4.
        // Then the 2-tick rule was mid-count and is reset: it fires on 6 and 8,
        // the 5-tick rule on 9.
        assert_eq!(trigger_indices(&mut rule, &trades(10)), vec![4, 9]);
    }

    #[test]
    fn all_rule_of_the_same_rule_twice_equals_the_rule() {
        let mut rule = AllRule::new(TickRule::new(3), TickRule::new(3));
        assert_eq!(trigger_indices(&mut rule, &trades(9)), vec![2, 5, 8]);
    }

    #[test]
    fn min_duration_delays_the_trigger() {
        // The inner rule fires on every trade; candles must last at least 4 ms.
        let mut rule = MinDuration::new(TickRule::new(1), MillisecondPeriod::from_non_zero(4));
        assert_eq!(trigger_indices(&mut rule, &trades(13)), vec![4, 8, 12]);
    }

    #[test]
    fn min_duration_does_not_trigger_without_the_inner_rule() {
        let mut rule = MinDuration::new(TickRule::new(100), MillisecondPeriod::from_non_zero(1));
        assert!(trigger_indices(&mut rule, &trades(50)).is_empty());
    }

    #[test]
    fn max_duration_closes_quiet_candles() {
        let mut rule = MaxDuration::new(TickRule::new(100), MillisecondPeriod::from_non_zero(4));
        // A candle opened at t contains the trades before t + 4 ms.
        assert_eq!(trigger_indices(&mut rule, &trades(13)), vec![4, 8, 12]);
    }

    #[test]
    fn max_duration_resets_the_inner_rule_on_timeout() {
        let mut rule = MaxDuration::new(TickRule::new(6), MillisecondPeriod::from_non_zero(4));
        // Without the reset, the tick rule would fire on 5 and close a 1-trade candle.
        assert_eq!(trigger_indices(&mut rule, &trades(13)), vec![4, 8, 12]);
    }

    #[test]
    fn max_duration_keeps_the_inner_trigger() {
        let mut rule = MaxDuration::new(TickRule::new(2), MillisecondPeriod::from_non_zero(100));
        assert_eq!(trigger_indices(&mut rule, &trades(6)), vec![1, 3, 5]);
    }

    #[test]
    fn max_duration_respects_the_timestamp_resolution() {
        #[derive(Debug, Clone, Copy)]
        struct MicroTrade(i64);
        impl Timestamped for MicroTrade {
            const TIMESTAMP_RESOLUTION: crate::TimestampResolution =
                crate::TimestampResolution::Microsecond;
            fn timestamp(&self) -> i64 {
                self.0
            }
        }
        /// A rule that never triggers, for any event type.
        struct Never;
        impl<E> AggregationRule<E> for Never {
            fn should_trigger(&mut self, _event: &E) -> bool {
                false
            }
        }
        let mut rule = MaxDuration::new(Never, MillisecondPeriod::from_non_zero(1));
        let triggers: Vec<i64> = (0..3_000)
            .step_by(500)
            .filter(|ts| rule.should_trigger(&MicroTrade(*ts)))
            .collect();
        // 1 ms is 1000 us: the candle opened at 0 closes at 1000 (500 still fits).
        assert_eq!(triggers, vec![1_000, 2_000]);
    }

    #[test]
    fn any_rule_closes_at_the_earlier_deadline_and_resets_the_other_rule() {
        let mut rule = AnyRule::new(
            TickRule::new(3),
            crate::AlignedTimeRule::new(MillisecondPeriod::from_non_zero(10)),
        );
        let t = trades(4);
        assert!(trigger_indices(&mut rule, &t[..2]).is_empty());
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(10));
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(20));
        // The tick rule forgot the two trades of the closed candle:
        // it fires on the third trade after the deadline, not on the first.
        let after: Vec<Trade> = (12..15).map(|ts| trade(ts, 100.0, 1.0)).collect();
        assert_eq!(trigger_indices(&mut rule, &after), vec![2]);
    }

    #[test]
    fn max_duration_has_a_deadline_after_the_first_trade() {
        let mut rule = MaxDuration::new(TickRule::new(100), MillisecondPeriod::from_non_zero(5));
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), None);
        assert!(trigger_indices(&mut rule, &trades(1)).is_empty());
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(5));
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(10));
        AggregationRule::<Trade>::skip_to(&mut rule, 33);
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), None);
    }

    #[test]
    fn min_duration_has_a_deadline_only_with_a_pending_trigger() {
        let mut rule = MinDuration::new(TickRule::new(2), MillisecondPeriod::from_non_zero(10));
        assert!(trigger_indices(&mut rule, &trades(1)).is_empty());
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), None);
        assert!(trigger_indices(&mut rule, &[trade(1, 100.0, 1.0)]).is_empty());
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(10));
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), None);
    }

    #[test]
    fn all_rule_has_the_deadline_of_the_rule_that_did_not_trigger_yet() {
        let mut rule = AllRule::new(
            TickRule::new(1),
            crate::AlignedTimeRule::new(MillisecondPeriod::from_non_zero(10)),
        );
        assert!(trigger_indices(&mut rule, &trades(1)).is_empty());
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), Some(10));
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(AggregationRule::<Trade>::deadline(&rule), None);
    }

    #[test]
    fn any_rule_last_trigger_is_none_before_any_close() {
        let rule = AnyRule::new(TickRule::new(3), TickRule::new(4));
        assert_eq!(rule.last_trigger(), None);
    }

    #[test]
    fn any_rule_last_trigger_reports_which_rule_fired() {
        let mut sizes = [1.0; 12];
        sizes[1] = 10.0;
        let trades: Vec<Trade> = sizes
            .iter()
            .zip(0..)
            .map(|(size, ts)| trade(ts, 100.0, *size))
            .collect();
        let mut rule = AnyRule::new(
            TickRule::new(5),
            VolumeRule::new(10.0, By::Quote).expect("valid threshold"),
        );
        // The volume rule (b) fires alone on trade 1, then the tick rule (a) alone on 6 and 11.
        let mut closes = Vec::new();
        for t in &trades {
            if rule.should_trigger(t) {
                closes.push(rule.last_trigger());
            }
        }
        assert_eq!(
            closes,
            vec![Some(Fired::Second), Some(Fired::First), Some(Fired::First)]
        );
    }

    #[test]
    fn any_rule_last_trigger_is_both_when_the_rules_fire_together() {
        let mut rule = AnyRule::new(TickRule::new(3), TickRule::new(3));
        let t = trades(3);
        assert!(!rule.should_trigger(&t[0]));
        assert!(!rule.should_trigger(&t[1]));
        assert!(rule.should_trigger(&t[2]));
        assert_eq!(rule.last_trigger(), Some(Fired::Both));
    }

    #[test]
    fn any_rule_last_trigger_at_a_deadline_identifies_the_rule_whose_deadline_it_was() {
        let mut rule = AnyRule::new(
            TickRule::new(3),
            crate::AlignedTimeRule::new(MillisecondPeriod::from_non_zero(10)),
        );
        assert!(trigger_indices(&mut rule, &trades(2)).is_empty());
        assert_eq!(rule.last_trigger(), None, "no close yet");
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(
            rule.last_trigger(),
            Some(Fired::Second),
            "b is the time rule"
        );
    }

    #[test]
    fn any_rule_last_trigger_survives_reset() {
        let mut rule = AnyRule::new(TickRule::new(1), TickRule::new(1));
        assert!(rule.should_trigger(&trade(0, 100.0, 1.0)));
        assert_eq!(rule.last_trigger(), Some(Fired::Both));
        AggregationRule::<Trade>::reset(&mut rule);
        assert_eq!(
            rule.last_trigger(),
            Some(Fired::Both),
            "reset forgets pending accumulators, not the record of the last close"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn any_rule_last_trigger_round_trips_with_serde() {
        let mut rule = AnyRule::new(TickRule::new(5), TickRule::new(3));
        assert_eq!(trigger_indices(&mut rule, &trades(3)), vec![2]);
        assert_eq!(rule.last_trigger(), Some(Fired::Second));
        let json = serde_json::to_string(&rule).expect("serializable");
        let restored: AnyRule<TickRule, TickRule> =
            serde_json::from_str(&json).expect("deserializable");
        assert_eq!(restored.last_trigger(), Some(Fired::Second));
    }

    #[test]
    fn all_rule_last_trigger_is_none_before_any_close() {
        let rule = AllRule::new(TickRule::new(1), TickRule::new(1));
        assert_eq!(rule.last_trigger(), None);
    }

    #[test]
    fn all_rule_last_trigger_reports_the_rule_that_completed_the_pair() {
        // The 5-tick rule completes the pair after the 2-tick rule already waited.
        let mut rule = AllRule::new(TickRule::new(2), TickRule::new(5));
        let mut closes = Vec::new();
        for t in &trades(10) {
            if rule.should_trigger(t) {
                closes.push(rule.last_trigger());
            }
        }
        assert_eq!(closes, vec![Some(Fired::Second), Some(Fired::Second)]);

        // Swapping the rules swaps which one is reported as completing the pair.
        let mut rule = AllRule::new(TickRule::new(5), TickRule::new(2));
        let mut closes = Vec::new();
        for t in &trades(10) {
            if rule.should_trigger(t) {
                closes.push(rule.last_trigger());
            }
        }
        assert_eq!(closes, vec![Some(Fired::First), Some(Fired::First)]);
    }

    #[test]
    fn all_rule_last_trigger_at_a_deadline_identifies_the_pending_rule() {
        let mut rule = AllRule::new(
            TickRule::new(1),
            crate::AlignedTimeRule::new(MillisecondPeriod::from_non_zero(10)),
        );
        assert!(trigger_indices(&mut rule, &trades(1)).is_empty());
        assert_eq!(rule.last_trigger(), None, "no close yet");
        AggregationRule::<Trade>::on_deadline(&mut rule);
        assert_eq!(
            rule.last_trigger(),
            Some(Fired::Second),
            "the time rule (b) was pending and completes the pair at its deadline"
        );
    }

    #[test]
    fn all_rule_last_trigger_survives_reset() {
        let mut rule = AllRule::new(TickRule::new(1), TickRule::new(1));
        assert!(rule.should_trigger(&trade(0, 100.0, 1.0)));
        assert_eq!(rule.last_trigger(), Some(Fired::Both));
        AggregationRule::<Trade>::reset(&mut rule);
        assert_eq!(rule.last_trigger(), Some(Fired::Both));
    }

    #[test]
    fn the_inner_rules_can_be_read() {
        let any = AnyRule::new(TickRule::new(2), TickRule::new(3));
        assert_eq!(any.first().threshold(), 2.0);
        assert_eq!(any.second().threshold(), 3.0);
        let all = AllRule::new(TickRule::new(4), TickRule::new(5));
        assert_eq!(all.first().threshold(), 4.0);
        assert_eq!(all.second().threshold(), 5.0);
        let min = MinDuration::new(TickRule::new(6), MillisecondPeriod::from_non_zero(1));
        assert_eq!(min.inner().threshold(), 6.0);
        let max = MaxDuration::new(TickRule::new(7), MillisecondPeriod::from_non_zero(1));
        assert_eq!(max.inner().threshold(), 7.0);
    }
}
