//! Calibrate the threshold of a rule to a target candle duration.

use crate::{AggregationRule, Error, MillisecondPeriod, Result, Timestamped};

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

/// How the activity that closes a candle grows with the duration of the candle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Scaling {
    /// In proportion to the duration, e.g. the volume or the number of trades:
    /// `activity ~ duration`.
    #[default]
    Linear,
    /// With the square root of the duration, e.g. the net taker volume of a
    /// [`SignedVolumeRule`](crate::SignedVolumeRule) or a price move, which are random walks:
    /// `activity^2 ~ duration`.
    Diffusive,
}

impl Scaling {
    /// The power of the activity that grows in proportion to the duration.
    fn exponent(self) -> i32 {
        match self {
            Scaling::Linear => 1,
            Scaling::Diffusive => 2,
        }
    }
}

/// Recalibrates the threshold of the inner rule after each of its triggers,
/// so that its candles last `target_duration` on average whatever the activity of the market.
///
/// A fixed threshold drifts with the regime: a tick or volume candle that lasts an hour
/// in a quiet year may last minutes in a busy one.
/// Following López de Prado (Advances in Financial Machine Learning, ch. 2),
/// `Adaptive` keeps an exponentially weighted moving average of
/// `duration / threshold^p` over the candles, an estimate of the inverse rate of activity
/// that does not depend on the current threshold, and sets the next threshold to
/// `(target_duration / average)^(1/p)`, clamped to `[min_threshold, max_threshold]`.
/// `p` is 1 for [`Scaling::Linear`] (the default) and 2 for [`Scaling::Diffusive`].
/// The duration of a candle is the time since the previous trigger of the inner rule.
///
/// Averaging the duration per activity, not the activity per duration, aims at the
/// arithmetic mean of the candle durations: `target_duration` = 1 minute means about
/// 1440 candles per day. When the durations are skewed (bursts close candles in milliseconds,
/// calm periods hold one for hours), averaging the rate would aim at the harmonic mean
/// and produce far fewer candles.
///
/// Notes:
/// - Choose the [`Scaling`] of the inner rule with [`Self::with_scaling`]:
///   with linear scaling, a rule whose activity grows with the square root of time
///   oscillates instead of settling.
/// - When a combinator closes the candle (e.g. [`MaxDuration`](crate::MaxDuration)),
///   the inner rule restarts ([`AggregationRule::reset`]).
///   Wrap the combinator around `Adaptive` (`MaxDuration<Adaptive<R>>`), not the other way round.
/// - A candle that a combinator closes before the inner rule triggered did not reach the
///   threshold: its activity per duration was lower than the threshold implies.
///   If it lasted longer than the average implies, `Adaptive` raises the average to
///   `duration / threshold^p`, a lower bound of the duration per activity (a censored sample),
///   which lowers the threshold. Without this, a threshold that the inner rule cannot reach
///   within a [`MaxDuration`](crate::MaxDuration) would never come down: the timeout restarts
///   the inner rule before it triggers, and only triggers calibrate. This happens after a
///   seed far above the activity, after one very short first candle (the first sample sets
///   the average alone), and when the activity drops by more than the ratio of the maximum
///   to the target duration. The next candle is then timed from its own first event,
///   so the time of the unfinished candle counts once.
/// - Otherwise, the duration keeps running from the previous trigger of the inner rule
///   when a combinator closes a candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Adaptive<R> {
    inner: R,
    target_duration: MillisecondPeriod,
    alpha: f64,
    min_threshold: f64,
    max_threshold: f64,
    scaling: Scaling,
    /// The start of the duration of the next sample: the last trigger of the inner rule,
    /// or the first event after a censored sample.
    last_trigger_timestamp: Option<i64>,
    /// The moving average of `duration in ms / threshold^p`.
    ms_per_activity: Option<f64>,
    /// The first event that the inner rule saw since it last started a candle:
    /// its last trigger, or the first event after a reset.
    candle_start_timestamp: Option<i64>,
    /// The latest event.
    latest_timestamp: Option<i64>,
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
            scaling: Scaling::Linear,
            last_trigger_timestamp: None,
            ms_per_activity: None,
            candle_start_timestamp: None,
            latest_timestamp: None,
        })
    }
}

impl<R> Adaptive<R> {
    /// Set how the activity of the inner rule grows with time, [`Scaling::Linear`] by default.
    pub fn with_scaling(self, scaling: Scaling) -> Self {
        Self { scaling, ..self }
    }

    /// The inner rule, e.g. to read its current threshold.
    pub fn inner(&self) -> &R {
        &self.inner
    }
}

/// The next threshold from the candle that just closed.
///
/// # Arguments:
/// `previous`: The moving average of `ms / activity^p` before this candle.
/// `candle_activity`: The threshold that the candle reached.
/// `candle_ms`: The duration of the candle.
///
/// # Returns:
/// The new moving average and the next threshold.
fn calibrate(
    previous: Option<f64>,
    (candle_activity, candle_ms): (f64, f64),
    (alpha, scaling): (f64, Scaling),
    target_ms: f64,
    (min_threshold, max_threshold): (f64, f64),
) -> (f64, f64) {
    assert!(candle_ms > 0.0, "a candle takes time");
    assert!(
        candle_activity > 0.0,
        "every threshold rule rejects thresholds <= 0"
    );
    assert!(candle_activity.is_finite());
    let p = scaling.exponent();
    let sample = candle_ms / candle_activity.powi(p);
    let average = previous.map_or(sample, |prev| alpha * sample + (1.0 - alpha) * prev);
    debug_assert!(average > 0.0);
    let threshold = threshold_for(average, scaling, target_ms, (min_threshold, max_threshold));
    (average, threshold)
}

/// The average and the threshold after a candle that the inner rule did not trigger,
/// if they change.
///
/// # Arguments:
/// `previous`: The moving average of `ms / activity^p`, `None` before the first trigger.
/// `threshold`: The threshold that the candle did not reach.
/// `open_ms`: How long the inner rule accumulated without reaching it.
///
/// # Returns:
/// The censored average and the lower threshold, or `None` if the candle did not last
/// longer than the average (or, before the first trigger, the threshold) implies.
fn censor(
    previous: Option<f64>,
    (threshold, open_ms): (f64, f64),
    scaling: Scaling,
    target_ms: f64,
    bounds: (f64, f64),
) -> Option<(f64, f64)> {
    assert!(
        threshold > 0.0,
        "every threshold rule rejects thresholds <= 0"
    );
    assert!(threshold.is_finite());
    assert!(open_ms >= 0.0, "a candle cannot end before it started");
    let p = scaling.exponent();
    // The activity stayed below the threshold, so the duration per activity was longer.
    let lower_bound = open_ms / threshold.powi(p);
    // Before the first trigger, the threshold itself implies the average.
    let current = previous.unwrap_or(target_ms / threshold.powi(p));
    if lower_bound <= current {
        return None;
    }
    let next = threshold_for(lower_bound, scaling, target_ms, bounds);
    // A larger average never gives a larger threshold, also at the bounds.
    assert!(
        next <= threshold,
        "a censored sample raised {threshold} to {next}"
    );
    Some((lower_bound, next))
}

/// The threshold that gives candles of `target_ms` at `average` ms per activity^p.
fn threshold_for(
    average: f64,
    scaling: Scaling,
    target_ms: f64,
    (min_threshold, max_threshold): (f64, f64),
) -> f64 {
    debug_assert!(average > 0.0);
    let unclamped = match scaling {
        Scaling::Linear => target_ms / average,
        Scaling::Diffusive => (target_ms / average).sqrt(),
    };
    let threshold = unclamped.clamp(min_threshold, max_threshold);
    debug_assert!(threshold >= min_threshold);
    debug_assert!(threshold <= max_threshold);
    threshold
}

impl<E, R> AggregationRule<E> for Adaptive<R>
where
    E: Timestamped,
    R: AggregationRule<E> + ThresholdRule,
{
    fn should_trigger(&mut self, trade: &E) -> bool {
        let start = *self.last_trigger_timestamp.get_or_insert(trade.timestamp());
        self.candle_start_timestamp.get_or_insert(trade.timestamp());
        self.latest_timestamp = Some(trade.timestamp());
        let trigger = self.inner.should_trigger(trade);
        if trigger {
            let units_per_ms = E::TIMESTAMP_RESOLUTION.units_per_millisecond();
            // Trades with the same timestamp still took some time: count at least one unit.
            let elapsed_units = trade.timestamp().saturating_sub(start).max(1);
            let candle_ms = elapsed_units as f64 / units_per_ms as f64;
            let target_ms = self.target_duration.get() as f64;
            let (average, threshold) = calibrate(
                self.ms_per_activity,
                (self.inner.threshold(), candle_ms),
                (self.alpha, self.scaling),
                target_ms,
                (self.min_threshold, self.max_threshold),
            );
            self.ms_per_activity = Some(average);
            self.inner.set_threshold(threshold).expect(
                "`Adaptive::new` checked that the inner rule accepts every clamped threshold",
            );
            self.last_trigger_timestamp = Some(trade.timestamp());
            self.candle_start_timestamp = Some(trade.timestamp());
        }
        trigger
    }

    /// A combinator closed the candle before the inner rule triggered:
    /// learn from it if it lasted too long (see the notes on [`Adaptive`]), and restart the inner rule.
    fn reset(&mut self) {
        self.inner.reset();
        if let (Some(start), Some(latest)) = (self.candle_start_timestamp, self.latest_timestamp) {
            let units_per_ms = E::TIMESTAMP_RESOLUTION.units_per_millisecond();
            let open_ms = latest.saturating_sub(start) as f64 / units_per_ms as f64;
            let censored = censor(
                self.ms_per_activity,
                (self.inner.threshold(), open_ms),
                self.scaling,
                self.target_duration.get() as f64,
                (self.min_threshold, self.max_threshold),
            );
            if let Some((average, threshold)) = censored {
                self.ms_per_activity = Some(average);
                self.inner.set_threshold(threshold).expect(
                    "`Adaptive::new` checked that the inner rule accepts every clamped threshold",
                );
                // The time of this candle is in the average now; time the next one from its start.
                self.last_trigger_timestamp = None;
            }
        }
        self.candle_start_timestamp = None;
    }

    fn deadline(&self) -> Option<i64> {
        self.inner.deadline()
    }

    fn on_deadline(&mut self) {
        self.inner.on_deadline();
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.inner.skip_to(timestamp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{random_flow, trade, trigger_indices},
        candle_components::NumTrades,
        Aggregator, BarKind, By, Candle, GenericAggregator, MaxDuration, SignedVolumeRule,
        TickRule, Trade, VolumeRule,
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
        // The timestamps are whole milliseconds with ten trades each,
        // so a candle of 100 ms measures 99 to 100 ms: allow 1%.
        trigger_indices(&mut rule, &trades_at_rate(100_000, 10, 2_000));
        let threshold = rule.inner().threshold();
        assert!((threshold - 1_000.0).abs() <= 10.0, "threshold {threshold}");
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
    fn calibrate_averages_the_duration_per_activity() {
        let bounds = (0.0, f64::MAX);
        let linear = (0.5, Scaling::Linear);
        let (first, threshold) = calibrate(None, (10.0, 10.0), linear, 100.0, bounds);
        assert_eq!(first, 1.0);
        assert_eq!(threshold, 100.0);
        // Three times the activity: 1/3 ms per unit, averaged with 1 to 2/3.
        let (second, threshold) = calibrate(Some(first), (30.0, 10.0), linear, 100.0, bounds);
        assert!((second - 2.0 / 3.0).abs() < 1e-12);
        assert!((threshold - 150.0).abs() < 1e-9);
    }

    #[test]
    fn calibrate_diffusive_sets_the_square_root() {
        let bounds = (0.0, f64::MAX);
        // A net flow of 10 in 400 ms: 4 ms per squared unit, so 100 ms take a net flow of 5.
        let (average, threshold) = calibrate(
            None,
            (10.0, 400.0),
            (0.5, Scaling::Diffusive),
            100.0,
            bounds,
        );
        assert_eq!(average, 4.0);
        assert_eq!(threshold, 5.0);
    }

    /// Adaptive delta bars as computed before they moved into this crate:
    /// the moving average of `ms / threshold^2` and `threshold = sqrt(target / average)`.
    fn reference_delta_thresholds(trades: &[Trade], target_ms: f64, seed: f64) -> Vec<f64> {
        let (lo, hi) = (seed / 100.0, seed * 100.0);
        let mut threshold = seed;
        let mut net = 0.0;
        let mut start: Option<i64> = None;
        let mut mean: Option<f64> = None;
        let mut out = Vec::new();
        for t in trades {
            let begin = *start.get_or_insert(t.timestamp);
            net += t.size;
            if net.abs() < threshold {
                continue;
            }
            net = 0.0;
            let bar_ms = (t.timestamp - begin).max(1) as f64;
            let sample = bar_ms / (threshold * threshold);
            let m = mean.map_or(sample, |m| 0.1 * sample + 0.9 * m);
            mean = Some(m);
            threshold = (target_ms / m).sqrt().clamp(lo, hi);
            start = Some(t.timestamp);
            out.push(threshold);
        }
        out
    }

    #[test]
    fn diffusive_signed_volume_matches_the_reference_pacing() {
        let trades = random_flow(200_000, 0.5, 42);
        let seed = 5.0;
        let mut rule = Adaptive::new(
            SignedVolumeRule::new(seed, By::Quote).expect("valid threshold"),
            MillisecondPeriod::from_non_zero(400),
            0.1,
            seed / 100.0,
            seed * 100.0,
        )
        .expect("valid parameters")
        .with_scaling(Scaling::Diffusive);
        let mut thresholds = Vec::new();
        for t in &trades {
            if rule.should_trigger(t) {
                thresholds.push(rule.inner().threshold());
            }
        }
        let reference = reference_delta_thresholds(&trades, 400.0, seed);
        assert!(thresholds.len() > 100);
        assert_eq!(thresholds, reference);
    }

    #[test]
    fn diffusive_pacing_hits_the_mean_duration_of_net_flow_bars() {
        // One trade per millisecond, buys and sells equally likely.
        let trades = random_flow(400_000, 0.5, 7);
        let mut rule = Adaptive::new(
            SignedVolumeRule::new(1.0, By::Quote).expect("valid threshold"),
            MillisecondPeriod::from_non_zero(400),
            0.1,
            0.1,
            1_000.0,
        )
        .expect("valid parameters")
        .with_scaling(Scaling::Diffusive);
        let triggers = trigger_indices(&mut rule, &trades);
        // Skip the calibration at the start.
        let settled = &triggers[10..];
        let mean_ms = (settled[settled.len() - 1] - settled[0]) as f64 / (settled.len() - 1) as f64;
        assert!(
            (300.0..500.0).contains(&mean_ms),
            "mean duration {mean_ms} ms"
        );
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
        AggregationRule::<Trade>::reset(&mut rule);
        assert_eq!(trigger_indices(&mut rule, &t[2..]), vec![2, 5]);
    }

    #[test]
    fn censor_only_lowers_the_threshold() {
        let bounds = (1.0, 1e9);
        let linear = Scaling::Linear;
        // 1 ms per unit: 100 units per candle of 100 ms.
        // Below 100 units for 50 ms says nothing new.
        assert_eq!(
            censor(Some(1.0), (100.0, 50.0), linear, 100.0, bounds),
            None
        );
        // Below 100 units for 1000 ms: at least 10 ms per unit, at most 10 units per 100 ms.
        assert_eq!(
            censor(Some(1.0), (100.0, 1_000.0), linear, 100.0, bounds),
            Some((10.0, 10.0))
        );
        // Before the first trigger, the threshold implies the average.
        assert_eq!(censor(None, (100.0, 100.0), linear, 100.0, bounds), None);
        assert_eq!(
            censor(None, (100.0, 400.0), linear, 100.0, bounds),
            Some((4.0, 25.0))
        );
        // A net flow below 10 for 400 ms: at least 4 ms per squared unit, a flow of 5 per 100 ms.
        assert_eq!(
            censor(None, (10.0, 400.0), Scaling::Diffusive, 100.0, bounds),
            Some((4.0, 5.0))
        );
        // The bounds still hold.
        assert_eq!(
            censor(Some(1.0), (100.0, 1e6), linear, 100.0, (50.0, 1e9)),
            Some((1e4, 50.0))
        );
    }

    /// A threshold far above the activity, so that the inner rule cannot trigger before
    /// `MaxDuration` restarts it: one trade per millisecond, a target of 100 trades per candle,
    /// a first threshold of a million trades and a timeout of one second.
    fn unreachable_threshold() -> MaxDuration<Adaptive<TickRule>> {
        let adaptive =
            Adaptive::new(TickRule::new(1_000_000), TARGET, 0.1, 1.0, 1e9).expect("valid");
        MaxDuration::new(adaptive, MillisecondPeriod::from_non_zero(1_000))
    }

    #[test]
    fn a_timeout_lowers_a_threshold_that_the_inner_rule_cannot_reach() {
        let mut rule = unreachable_threshold();
        let triggers = trigger_indices(&mut rule, &trades_at_rate(20_000, 1, 0));
        let threshold = rule.inner().inner().threshold();
        assert!((90.0..=110.0).contains(&threshold), "threshold {threshold}");
        // At the end the inner rule closes candles of about 100 trades, not the timeout of 1000.
        let last = &triggers[triggers.len() - 10..];
        assert!(last.windows(2).all(|w| w[1] - w[0] < 120), "{last:?}");
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct CountCandle {
        num_trades: NumTrades<u32>,
    }

    #[test]
    fn a_deadline_lowers_a_threshold_that_the_inner_rule_cannot_reach() {
        // One trade every 10 ms: a target of 10 trades per candle. The timeout passes at the
        // deadline of `MaxDuration`, before the trade at that time.
        let trades: Vec<Trade> = (0..5_000).map(|i| trade(i * 10, 100.0, 1.0)).collect();
        let mut aggregator =
            GenericAggregator::<CountCandle, _, Trade>::new(unreachable_threshold(), false);
        let mut bars = Vec::new();
        for t in &trades {
            aggregator.update(t, &mut bars);
        }
        assert_eq!(bars[0].kind(), BarKind::Deadline);
        let threshold = aggregator.rule().inner().inner().threshold();
        assert!((9.0..=11.0).contains(&threshold), "threshold {threshold}");
        let last = &bars[bars.len() - 10..];
        assert!(last.iter().all(|b| b.kind() == BarKind::Rule), "{last:?}");
        assert!(last.iter().all(|b| b.candle().num_trades() <= 11));
    }

    #[test]
    fn a_short_reset_keeps_the_threshold_and_the_running_duration() {
        let mut rule = adaptive_ticks(1.0, 1.0, 1e9);
        let t = trades_at_rate(1_000, 1, 0);
        assert_eq!(trigger_indices(&mut rule, &t[..301]), [0, 100, 200, 300]);
        assert_eq!(rule.inner().threshold(), 100.0);
        // A combinator closes the candle after 50 of 100 trades: no censored sample.
        assert!(trigger_indices(&mut rule, &t[301..351]).is_empty());
        AggregationRule::<Trade>::reset(&mut rule);
        assert_eq!(rule.inner().threshold(), 100.0);
        // The duration of the next sample still runs from the last trigger at 300 ms:
        // the inner rule restarts at 351 and triggers at 450, so the sample is 150 ms
        // per 100 trades and the threshold drops to 100 / 1.5.
        assert_eq!(trigger_indices(&mut rule, &t[351..451]), [99]);
        let threshold = rule.inner().threshold();
        assert!(
            (threshold - 100.0 / 1.5).abs() < 1.0,
            "threshold {threshold}"
        );
    }
}
