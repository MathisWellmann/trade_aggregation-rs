//! Calibrate the threshold of a rule to a target candle duration.

use crate::{AggregationRule, Error, MillisecondPeriod, Result, TakerTrade};

/// A rule whose trigger threshold can be read and changed, so that [`Adaptive`] can calibrate it.
///
/// The threshold is the amount of activity a candle accumulates before the rule triggers,
/// e.g. the number of trades of a [`TickRule`](crate::TickRule)
/// or the volume of a [`VolumeRule`](crate::VolumeRule).
pub trait ThresholdRule {
    /// The current threshold.
    fn threshold(&self) -> f64;

    /// Change the threshold, effective for the current candle.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if the rule does not accept `threshold`, leaving it unchanged.
    fn set_threshold(&mut self, threshold: f64) -> Result<()>;
}

/// Recalibrates the threshold of the inner rule after each of its triggers,
/// so that its candles last about `target_duration` whatever the activity of the market.
///
/// A fixed threshold drifts with the regime: a tick or volume candle that lasts an hour
/// in a quiet year may last minutes in a busy one.
/// Following López de Prado (Advances in Financial Machine Learning, ch. 2),
/// `Adaptive` estimates the activity per millisecond with an exponentially weighted
/// moving average and sets the threshold to `activity per millisecond * target duration`,
/// clamped to `[min_threshold, max_threshold]`.
/// The activity of a candle is the threshold it reached and its duration is the time
/// since the previous trigger of the inner rule.
///
/// Notes:
/// - The estimate assumes that activity grows linearly with time, as volume and trade counts do.
///   Price based rules grow more slowly (about with the square root of time);
///   use a small `alpha` (e.g. 0.1) with them so that the calibration settles instead of oscillating.
/// - When a combinator closes the candle (e.g. [`MaxDuration`](crate::MaxDuration)),
///   the inner rule restarts but the duration keeps running from its previous trigger.
///   Slow inner triggers therefore lower the threshold.
///   Wrap the combinator around `Adaptive` (`MaxDuration<Adaptive<R>>`), not the other way round.
#[derive(Debug, Clone)]
pub struct Adaptive<R> {
    inner: R,
    target_duration: MillisecondPeriod,
    alpha: f64,
    min_threshold: f64,
    max_threshold: f64,
    last_trigger_timestamp: Option<i64>,
    activity_per_ms: Option<f64>,
}

impl<R: ThresholdRule> Adaptive<R> {
    /// Wrap `inner`, calibrating its threshold to candles of about `target_duration`.
    ///
    /// # Arguments:
    /// `inner`: The rule to calibrate. Its current threshold is used until its first trigger,
    ///     clamped to `[min_threshold, max_threshold]`.
    /// `target_duration`: The duration each candle should have on average.
    /// `alpha`: The weight of the newest candle in the moving average of the activity, in `(0, 1]`.
    ///     Smaller values adapt more slowly and more smoothly.
    /// `min_threshold`, `max_threshold`: The bounds of the threshold, both accepted by `inner`.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `alpha` is not in `(0, 1]`, if the bounds are not finite,
    /// if `min_threshold > max_threshold`, or if `inner` rejects either bound.
    pub fn new(
        mut inner: R,
        target_duration: MillisecondPeriod,
        alpha: f64,
        min_threshold: f64,
        max_threshold: f64,
    ) -> Result<Self> {
        if !(alpha > 0.0 && alpha <= 1.0) {
            return Err(Error::InvalidParam);
        }
        if !min_threshold.is_finite() || !max_threshold.is_finite() {
            return Err(Error::InvalidParam);
        }
        if min_threshold > max_threshold {
            return Err(Error::InvalidParam);
        }
        // The valid thresholds of every rule form an interval,
        // so accepting both bounds means accepting every clamped threshold.
        let initial = inner.threshold().clamp(min_threshold, max_threshold);
        inner.set_threshold(min_threshold)?;
        inner.set_threshold(max_threshold)?;
        inner.set_threshold(initial)?;
        Ok(Self {
            inner,
            target_duration,
            alpha,
            min_threshold,
            max_threshold,
            last_trigger_timestamp: None,
            activity_per_ms: None,
        })
    }

    /// The inner rule, e.g. to read its current threshold.
    pub fn inner(&self) -> &R {
        &self.inner
    }
}

/// The next threshold from the activity of the candle that just closed.
///
/// # Returns:
/// The new moving average of the activity per millisecond and the next threshold.
fn calibrate(
    previous_activity_per_ms: Option<f64>,
    candle_activity: f64,
    candle_ms: f64,
    alpha: f64,
    target_ms: f64,
    (min_threshold, max_threshold): (f64, f64),
) -> (f64, f64) {
    debug_assert!(candle_ms > 0.0);
    debug_assert!(candle_activity.is_finite());
    let activity_per_ms = candle_activity / candle_ms;
    let average = previous_activity_per_ms.map_or(activity_per_ms, |prev| {
        alpha * activity_per_ms + (1.0 - alpha) * prev
    });
    let threshold = (average * target_ms).clamp(min_threshold, max_threshold);
    debug_assert!(threshold >= min_threshold);
    debug_assert!(threshold <= max_threshold);
    (average, threshold)
}

impl<C, T, R> AggregationRule<C, T> for Adaptive<R>
where
    T: TakerTrade,
    R: AggregationRule<C, T> + ThresholdRule,
{
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool {
        let start = *self.last_trigger_timestamp.get_or_insert(trade.timestamp());
        let trigger = self.inner.should_trigger(trade, candle);
        if trigger {
            let units_per_ms = trade.timestamp_resolution().units_per_millisecond();
            // Trades with the same timestamp still took some time: count at least one unit.
            let elapsed_units = trade.timestamp().saturating_sub(start).max(1);
            let candle_ms = elapsed_units as f64 / units_per_ms as f64;
            let target_ms = self.target_duration.get() as f64;
            let (average, threshold) = calibrate(
                self.activity_per_ms,
                self.inner.threshold(),
                candle_ms,
                self.alpha,
                target_ms,
                (self.min_threshold, self.max_threshold),
            );
            self.activity_per_ms = Some(average);
            self.inner.set_threshold(threshold).expect(
                "`Adaptive::new` checked that the inner rule accepts every clamped threshold",
            );
            self.last_trigger_timestamp = Some(trade.timestamp());
        }
        trigger
    }

    fn reset(&mut self) {
        self.inner.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, trigger_indices},
        By, TickRule, Trade, VolumeRule,
    };

    const TARGET: MillisecondPeriod = MillisecondPeriod::from_non_zero(100);

    /// `n` trades, `per_ms` of them in each millisecond, starting at `start_ms`.
    fn trades_at_rate(n: i64, per_ms: i64, start_ms: i64) -> Vec<Trade> {
        (0..n)
            .map(|i| trade(start_ms + i / per_ms, 100.0, 1.0))
            .collect()
    }

    fn adaptive_ticks(alpha: f64, min: f64, max: f64) -> Adaptive<TickRule> {
        Adaptive::new(TickRule::new(1), TARGET, alpha, min, max).expect("valid parameters")
    }

    #[test]
    fn new_rejects_invalid_parameters() {
        let new = |alpha, min, max| Adaptive::new(TickRule::new(1), TARGET, alpha, min, max);
        assert!(new(0.0, 1.0, 10.0).is_err());
        assert!(new(-0.5, 1.0, 10.0).is_err());
        assert!(new(1.5, 1.0, 10.0).is_err());
        assert!(new(f64::NAN, 1.0, 10.0).is_err());
        assert!(new(0.5, 10.0, 1.0).is_err());
        assert!(new(0.5, 0.0, 10.0).is_err());
        assert!(new(0.5, 1.0, f64::INFINITY).is_err());
        assert!(new(1.0, 1.0, 1.0).is_ok());
    }

    #[test]
    fn new_clamps_the_initial_threshold() {
        let rule = adaptive_ticks(0.5, 10.0, 20.0);
        assert_eq!(rule.inner().threshold(), 10.0);
    }

    #[test]
    fn constant_activity_calibrates_after_the_first_candle() {
        let mut rule = adaptive_ticks(0.5, 1.0, 1e9);
        // One trade per millisecond: 100 trades per 100 ms candle.
        let triggers = trigger_indices(&mut rule, &trades_at_rate(1_000, 1, 0));
        assert_eq!(rule.inner().threshold(), 100.0);
        assert_eq!(triggers[1..5], [100, 200, 300, 400]);
    }

    #[test]
    fn a_change_of_activity_moves_the_threshold() {
        let mut rule = adaptive_ticks(0.5, 1.0, 1e9);
        trigger_indices(&mut rule, &trades_at_rate(2_000, 1, 0));
        assert_eq!(rule.inner().threshold(), 100.0);
        // Ten times the activity: the threshold converges to 1000 trades per candle.
        trigger_indices(&mut rule, &trades_at_rate(100_000, 10, 2_000));
        let threshold = rule.inner().threshold();
        assert!((threshold - 1_000.0).abs() <= 1.0, "threshold {threshold}");
    }

    #[test]
    fn the_threshold_stays_within_its_bounds() {
        let mut rule = adaptive_ticks(1.0, 1.0, 50.0);
        trigger_indices(&mut rule, &trades_at_rate(1_000, 1, 0));
        assert_eq!(rule.inner().threshold(), 50.0);

        let mut rule = adaptive_ticks(1.0, 500.0, 1_000.0);
        trigger_indices(&mut rule, &trades_at_rate(10_000, 1, 0));
        assert_eq!(rule.inner().threshold(), 500.0);
    }

    #[test]
    fn calibrate_averages_the_activity() {
        let bounds = (0.0, f64::MAX);
        let (first, threshold) = calibrate(None, 10.0, 10.0, 0.5, 100.0, bounds);
        assert_eq!(first, 1.0);
        assert_eq!(threshold, 100.0);
        let (second, threshold) = calibrate(Some(first), 30.0, 10.0, 0.5, 100.0, bounds);
        assert_eq!(second, 2.0);
        assert_eq!(threshold, 200.0);
    }

    #[test]
    fn tick_rule_threshold_rounds_to_whole_trades() {
        let mut rule = TickRule::new(5);
        assert_eq!(rule.threshold(), 5.0);
        rule.set_threshold(7.4).expect("valid threshold");
        assert_eq!(rule.threshold(), 7.0);
        rule.set_threshold(0.2).expect("valid threshold");
        assert_eq!(rule.threshold(), 1.0);
        assert!(rule.set_threshold(0.0).is_err());
        assert!(rule.set_threshold(f64::NAN).is_err());
        assert_eq!(rule.threshold(), 1.0);
    }

    #[test]
    fn volume_rule_keeps_the_carried_volume_when_the_threshold_changes() {
        let mut rule = VolumeRule::new(10.0, By::Quote).expect("valid threshold");
        let trades = [
            trade(0, 100.0, 4.0),
            trade(1, 100.0, 8.0),
            trade(2, 100.0, 1.0),
            trade(3, 100.0, 18.0),
        ];
        // Fires on trade 1 with 12 of 10, carrying 2 into the next candle.
        assert_eq!(trigger_indices(&mut rule, &trades[..2]), vec![1]);
        rule.set_threshold(20.0).expect("valid threshold");
        // 2 carried + 1 + 18 = 21 >= 20.
        assert_eq!(trigger_indices(&mut rule, &trades[2..]), vec![1]);
        assert!(rule.set_threshold(-1.0).is_err());
        assert_eq!(rule.threshold(), 20.0);
    }

    #[test]
    fn reset_restarts_the_inner_rule() {
        let mut rule = adaptive_ticks(0.5, 3.0, 3.0);
        let t = trades_at_rate(10, 1, 0);
        assert!(trigger_indices(&mut rule, &t[..2]).is_empty());
        AggregationRule::<crate::plot::OhlcCandle, Trade>::reset(&mut rule);
        assert_eq!(trigger_indices(&mut rule, &t[2..]), vec![2, 5]);
    }
}
