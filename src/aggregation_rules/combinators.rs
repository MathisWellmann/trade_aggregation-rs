//! Rules built from other rules: either of two rules, both of two rules,
//! and lower and upper bounds on the duration of a candle.
//!
//! The duration of a candle is measured from the trade that triggered the previous candle
//! (or from the first trade) to the current trade.
//! With [`TriggerTrade::ClosingCandle`](crate::TriggerTrade::ClosingCandle) the candle opens one trade later,
//! so the measured duration is slightly longer than the candle.

use crate::{AggregationRule, MillisecondPeriod, TakerTrade};

/// Reset the inner rules that did not trigger on the current trade,
/// because a candle closed while they were still accumulating.
/// A rule that did trigger has already restarted its own state.
fn reset_untriggered<C, T, R>(rule: &mut R, triggered_now: bool)
where
    T: TakerTrade,
    R: AggregationRule<C, T>,
{
    if !triggered_now {
        rule.reset();
    }
}

/// The time from `open_timestamp` to the timestamp of `trade`, in the units of `trade`.
fn elapsed<T: TakerTrade>(open_timestamp: i64, trade: &T) -> i64 {
    let elapsed = trade.timestamp().saturating_sub(open_timestamp);
    debug_assert!(elapsed >= 0, "trades must arrive in time order");
    elapsed
}

/// Closes the candle as soon as either rule triggers,
/// e.g. a dollar volume rule or a volatility rule, whichever comes first ("hybrid bars").
///
/// Both rules see every trade.
/// When one rule closes the candle, the other one is [reset](AggregationRule::reset),
/// so that both start the next candle from scratch.
/// Nest `AnyRule`s to combine more than two rules.
#[derive(Debug, Clone)]
pub struct AnyRule<A, B> {
    a: A,
    b: B,
}

impl<A, B> AnyRule<A, B> {
    /// Combine two rules, of which either one closes the candle.
    pub fn new(a: A, b: B) -> Self {
        Self { a, b }
    }
}

impl<C, T, A, B> AggregationRule<C, T> for AnyRule<A, B>
where
    T: TakerTrade,
    A: AggregationRule<C, T>,
    B: AggregationRule<C, T>,
{
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool {
        // Evaluate both rules, so that neither misses a trade.
        let a = self.a.should_trigger(trade, candle);
        let b = self.b.should_trigger(trade, candle);
        let trigger = a || b;
        if trigger {
            reset_untriggered(&mut self.a, a);
            reset_untriggered(&mut self.b, b);
        }
        trigger
    }

    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
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
pub struct AllRule<A, B> {
    a: A,
    b: B,
    a_triggered: bool,
    b_triggered: bool,
}

impl<A, B> AllRule<A, B> {
    /// Combine two rules, which both have to trigger to close the candle.
    pub fn new(a: A, b: B) -> Self {
        Self {
            a,
            b,
            a_triggered: false,
            b_triggered: false,
        }
    }
}

impl<C, T, A, B> AggregationRule<C, T> for AllRule<A, B>
where
    T: TakerTrade,
    A: AggregationRule<C, T>,
    B: AggregationRule<C, T>,
{
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool {
        let a = self.a.should_trigger(trade, candle);
        let b = self.b.should_trigger(trade, candle);
        self.a_triggered |= a;
        self.b_triggered |= b;
        let trigger = self.a_triggered && self.b_triggered;
        if trigger {
            reset_untriggered(&mut self.a, a);
            reset_untriggered(&mut self.b, b);
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
}

/// Keeps a candle open for at least `min_duration`,
/// so that a burst of activity does not produce a flood of tiny candles.
///
/// If the inner rule triggers earlier, the trigger is remembered and the candle closes
/// at the first trade at least `min_duration` after the candle opened.
/// Further triggers of the inner rule in the meantime merge into that candle.
#[derive(Debug, Clone)]
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
}

impl<C, T, R> AggregationRule<C, T> for MinDuration<R>
where
    T: TakerTrade,
    R: AggregationRule<C, T>,
{
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool {
        let open_timestamp = *self.open_timestamp.get_or_insert(trade.timestamp());
        let inner = self.inner.should_trigger(trade, candle);
        self.pending |= inner;
        let min_units = self.min_duration.in_units(trade.timestamp_resolution());
        let trigger = self.pending && elapsed(open_timestamp, trade) >= min_units;
        if trigger {
            reset_untriggered(&mut self.inner, inner);
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
}

/// Closes a candle at the first trade more than `max_duration` after it opened,
/// unless the inner rule closed it before, so that quiet markets still produce candles.
///
/// The rule only sees trades, so a gap without trades produces no empty candles:
/// the candle closes at the first trade after the gap.
/// On a timeout the inner rule is [reset](AggregationRule::reset).
#[derive(Debug, Clone)]
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
}

impl<C, T, R> AggregationRule<C, T> for MaxDuration<R>
where
    T: TakerTrade,
    R: AggregationRule<C, T>,
{
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool {
        let open_timestamp = *self.open_timestamp.get_or_insert(trade.timestamp());
        let inner = self.inner.should_trigger(trade, candle);
        let max_units = self.max_duration.in_units(trade.timestamp_resolution());
        let timed_out = elapsed(open_timestamp, trade) > max_units;
        let trigger = inner || timed_out;
        if trigger {
            reset_untriggered(&mut self.inner, inner);
            self.open_timestamp = Some(trade.timestamp());
        }
        trigger
    }

    fn reset(&mut self) {
        self.inner.reset();
        self.open_timestamp = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, trigger_indices},
        By, TickRule, Trade, VolumeRule,
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
        AggregationRule::<crate::plot::OhlcCandle, Trade>::reset(&mut rule);
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
        let mut rule = MaxDuration::new(TickRule::new(100), MillisecondPeriod::from_non_zero(3));
        // A candle opened at t may contain trades up to t + 3 ms.
        assert_eq!(trigger_indices(&mut rule, &trades(13)), vec![4, 8, 12]);
    }

    #[test]
    fn max_duration_resets_the_inner_rule_on_timeout() {
        let mut rule = MaxDuration::new(TickRule::new(6), MillisecondPeriod::from_non_zero(3));
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
        impl TakerTrade for MicroTrade {
            fn timestamp(&self) -> i64 {
                self.0
            }
            fn timestamp_resolution(&self) -> crate::TimestampResolution {
                crate::TimestampResolution::Microsecond
            }
            fn price(&self) -> f64 {
                100.0
            }
            fn size(&self) -> f64 {
                1.0
            }
        }
        /// A rule that never triggers, for any candle and trade type.
        struct Never;
        impl<C, T: TakerTrade> AggregationRule<C, T> for Never {
            fn should_trigger(&mut self, _trade: &T, _candle: &C) -> bool {
                false
            }
        }
        let mut rule = MaxDuration::new(Never, MillisecondPeriod::from_non_zero(1));
        let triggers: Vec<i64> = (0..3_000)
            .step_by(500)
            .filter(|ts| rule.should_trigger(&MicroTrade(*ts), &()))
            .collect();
        // 1 ms is 1000 us: the candle opened at 0 closes at 1500 (1000 would still fit).
        assert_eq!(triggers, vec![1_500]);
    }
}
