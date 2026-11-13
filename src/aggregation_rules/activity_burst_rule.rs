use crate::{AggregationRule, AsTrade, Error, MillisecondPeriod, Result};

/// Splits the stream where the trade arrival regime changes:
/// a candle closes when a burst of trading starts and when it ends.
///
/// Trade arrivals cluster: trades trigger more trades, which Hawkes processes model as an
/// intensity that jumps at each trade and decays afterwards. Bursts of such intensity follow
/// news and other events (Rambaldi, Filimonov and Lillo 2018), and the market behaves
/// differently inside a burst, so a burst makes a natural candle.
///
/// The rule keeps two exponentially decaying trade intensities with time constants
/// `fast < slow`, updated at each trade as `lambda = lambda * exp(-dt / tau) + 1 / tau`,
/// in trades per millisecond whatever the timestamp resolution. Both are divided by
/// `1 - exp(-t / tau)`, with `t` the time since the first trade, to correct their start at zero.
/// The rule is calm or in a burst. It triggers when it is calm and `fast / slow >= enter_ratio`
/// (a burst starts), and when it is in a burst and `fast / slow <= exit_ratio` (the burst ends).
/// The gap between both ratios (hysteresis) keeps it from flickering around one level.
/// It does not trigger before `slow` has passed since the first trade (warm-up), because the
/// slow intensity has not seen enough trades yet; the regime stays calm until then.
///
/// The ratio is a ratio of rates, so the same parameters fit symbols with very different
/// trading rates. The ratio is evaluated at the trades only: the end of a burst is noticed at
/// the first trade after it. At the trades, the fast intensity includes the jump of the trade,
/// so in a steady regime the ratio is above 1 unless `fast` spans several trades.
/// With an `exit_ratio` near 1, a burst ends when the fast intensity is back at the level of the
/// slow one; a burst that lasts longer than about `slow` then becomes the new normal and ends too.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActivityBurstRule {
    fast: MillisecondPeriod,
    slow: MillisecondPeriod,
    enter_ratio: f64,
    exit_ratio: f64,
    /// The fast and slow intensities in trades per millisecond, before the start correction.
    intensities: [f64; 2],
    /// The timestamps of the first and the last trade.
    first_and_last: Option<(i64, i64)>,
    in_burst: bool,
}

impl ActivityBurstRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `fast`, `slow`: The time constants of the two trade intensities, e.g. 2 seconds and
    ///     1 minute. `fast` should span a few trades of a calm market and be shorter than the
    ///     bursts to detect.
    /// `enter_ratio`: The ratio `fast / slow` at or above which a burst starts, e.g. 4.
    /// `exit_ratio`: The ratio `fast / slow` at or below which a burst ends, e.g. 1.5.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `fast` is not shorter than `slow`, if a ratio is not finite,
    /// or unless `0 < exit_ratio < enter_ratio`.
    pub fn new(
        fast: MillisecondPeriod,
        slow: MillisecondPeriod,
        enter_ratio: f64,
        exit_ratio: f64,
    ) -> Result<Self> {
        if fast >= slow {
            return Err(Error::InvalidParam);
        }
        if !enter_ratio.is_finite() || !exit_ratio.is_finite() {
            return Err(Error::InvalidParam);
        }
        if !(0.0 < exit_ratio && exit_ratio < enter_ratio) {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            fast,
            slow,
            enter_ratio,
            exit_ratio,
            intensities: [0.0; 2],
            first_and_last: None,
            in_burst: false,
        })
    }

    /// Whether the trades are in a burst: after the trigger that started it
    /// and before the trigger that ends it.
    pub fn in_burst(&self) -> bool {
        self.in_burst
    }
}

/// The time constants of the fast and the slow intensity, in milliseconds.
fn time_constants(fast: MillisecondPeriod, slow: MillisecondPeriod) -> [f64; 2] {
    let taus = [fast.get() as f64, slow.get() as f64];
    debug_assert!(taus[0] > 0.0);
    debug_assert!(taus[0] < taus[1]);
    taus
}

/// The intensity after a trade `elapsed_ms` after the previous one.
fn intensity(previous: f64, elapsed_ms: f64, tau_ms: f64) -> f64 {
    debug_assert!(previous >= 0.0);
    debug_assert!(elapsed_ms >= 0.0);
    let next = previous * (-elapsed_ms / tau_ms).exp() + 1.0 / tau_ms;
    debug_assert!(next > 0.0);
    next
}

/// `fast / slow`, with each intensity corrected for its start at zero `since_first_ms` ago.
fn ratio([fast, slow]: [f64; 2], [fast_tau, slow_tau]: [f64; 2], since_first_ms: f64) -> f64 {
    assert!(
        since_first_ms > 0.0,
        "the correction needs time since the first trade"
    );
    let reached = |tau: f64| -(-since_first_ms / tau).exp_m1();
    let ratio = (fast / reached(fast_tau)) / (slow / reached(slow_tau));
    debug_assert!(ratio.is_finite());
    debug_assert!(ratio > 0.0);
    ratio
}

impl<E: AsTrade> AggregationRule<E> for ActivityBurstRule {
    fn should_trigger(&mut self, event: &E) -> bool {
        if event.as_trade().is_none() {
            return false;
        }
        let timestamp = event.timestamp();
        let (first, last) = self.first_and_last.unwrap_or((timestamp, timestamp));
        let elapsed = timestamp.saturating_sub(last);
        assert!(
            elapsed >= 0,
            "trades in time order: {timestamp} after {last}"
        );
        self.first_and_last = Some((first, timestamp));

        let units_per_ms = E::TIMESTAMP_RESOLUTION.units_per_millisecond() as f64;
        let taus = time_constants(self.fast, self.slow);
        let elapsed_ms = elapsed as f64 / units_per_ms;
        for (lambda, tau) in self.intensities.iter_mut().zip(taus) {
            *lambda = intensity(*lambda, elapsed_ms, tau);
        }

        let since_first = timestamp.saturating_sub(first);
        if since_first < self.slow.in_units(E::TIMESTAMP_RESOLUTION) {
            debug_assert!(!self.in_burst, "calm during the warm-up");
            return false;
        }
        let ratio = ratio(self.intensities, taus, since_first as f64 / units_per_ms);
        let switch = if self.in_burst {
            ratio <= self.exit_ratio
        } else {
            ratio >= self.enter_ratio
        };
        if switch {
            self.in_burst = !self.in_burst;
        }
        switch
    }

    /// Does nothing: the regime and the intensities follow the stream, not the candle,
    /// so a candle closed by another rule neither ends a burst nor starts a new warm-up.
    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, MicroTrade, NanoTrade},
        MarketEvent, Quote, Trade,
    };

    fn rule() -> ActivityBurstRule {
        ActivityBurstRule::new(
            MillisecondPeriod::from_non_zero_secs(2),
            MillisecondPeriod::from_non_zero_secs(60),
            4.0,
            1.5,
        )
        .expect("valid parameters")
    }

    /// Millisecond timestamps: 120 trades 1 s apart, a burst of 500 trades 10 ms apart
    /// (indices 120 to 619), then 120 trades 1 s apart again.
    fn calm_burst_calm() -> Vec<i64> {
        let calm = (0..120).map(|i| i * 1_000);
        let burst = (0..500).map(|i| 120_000 + i * 10);
        let calm_again = (1..=120).map(|i| 125_000 + i * 1_000);
        calm.chain(burst).chain(calm_again).collect()
    }

    fn fired<E: AsTrade>(rule: &mut ActivityBurstRule, events: &[E]) -> Vec<usize> {
        (0..events.len())
            .filter(|i| rule.should_trigger(&events[*i]))
            .collect()
    }

    fn trades(timestamps: &[i64]) -> Vec<Trade> {
        timestamps.iter().map(|ts| trade(*ts, 100.0, 1.0)).collect()
    }

    #[test]
    fn new_rejects_invalid_parameters() {
        let s = MillisecondPeriod::from_non_zero_secs;
        let new = |fast, slow, enter, exit| ActivityBurstRule::new(s(fast), s(slow), enter, exit);
        assert!(new(60, 60, 4.0, 1.5).is_err());
        assert!(new(61, 60, 4.0, 1.5).is_err());
        assert!(new(2, 60, 4.0, 4.0).is_err());
        assert!(new(2, 60, 1.5, 4.0).is_err());
        assert!(new(2, 60, 4.0, 0.0).is_err());
        assert!(new(2, 60, 4.0, -1.0).is_err());
        assert!(new(2, 60, f64::INFINITY, 1.5).is_err());
        assert!(new(2, 60, 4.0, f64::NAN).is_err());
        assert!(new(2, 60, 0.9, 0.5).is_ok());
    }

    #[test]
    fn triggers_when_a_burst_starts_and_ends() {
        let mut r = rule();
        let trades = trades(&calm_burst_calm());
        let fired = fired(&mut r, &trades);
        assert_eq!(fired.len(), 2, "{fired:?}");
        // The burst starts within its first 0.2 seconds ...
        assert!((120..140).contains(&fired[0]), "{fired:?}");
        // ... and ends within ten calm trades.
        assert!((620..630).contains(&fired[1]), "{fired:?}");
        assert!(!r.in_burst());
    }

    #[test]
    fn in_burst_follows_the_regime() {
        let mut r = rule();
        let trades = trades(&calm_burst_calm());
        let mut regimes = Vec::new();
        for t in &trades {
            let _ = r.should_trigger(t);
            regimes.push(r.in_burst());
        }
        assert!(!regimes[119]);
        assert!(regimes[200]);
        assert!(regimes[619]);
        assert!(!regimes[700]);
    }

    #[test]
    fn a_burst_during_the_warm_up_does_not_trigger() {
        // The burst starts with the stream: nothing to compare it with.
        let burst = (0..500).map(|i| i * 10);
        let calm = (1..=120).map(|i| 5_000 + i * 1_000);
        let timestamps: Vec<i64> = burst.chain(calm).collect();
        let mut r = rule();
        assert!(fired(&mut r, &trades(&timestamps)).is_empty());
        assert!(!r.in_burst());
    }

    #[test]
    fn a_steady_rate_never_triggers() {
        let timestamps: Vec<i64> = (0..10_000).map(|i| i * 100).collect();
        assert!(fired(&mut rule(), &trades(&timestamps)).is_empty());
    }

    #[test]
    fn follows_the_timestamp_resolution() {
        let timestamps = calm_burst_calm();
        let expected = fired(&mut rule(), &trades(&timestamps));
        let micros: Vec<MicroTrade> = timestamps
            .iter()
            .map(|ts| MicroTrade(trade(ts * 1_000, 100.0, 1.0)))
            .collect();
        let nanos: Vec<NanoTrade> = timestamps
            .iter()
            .map(|ts| NanoTrade(trade(ts * 1_000_000, 100.0, 1.0)))
            .collect();
        assert_eq!(fired(&mut rule(), &micros), expected);
        assert_eq!(fired(&mut rule(), &nanos), expected);
    }

    #[test]
    fn quotes_are_not_counted() {
        let timestamps = calm_burst_calm();
        let expected = fired(&mut rule(), &trades(&timestamps));
        // A quote before every trade, as often as the trades: they must not add intensity.
        let events: Vec<MarketEvent<Trade, Quote>> = timestamps
            .iter()
            .flat_map(|ts| {
                let quote = Quote {
                    timestamp: *ts,
                    ..Quote::default()
                };
                [
                    MarketEvent::Quote(quote),
                    MarketEvent::Trade(trade(*ts, 100.0, 1.0)),
                ]
            })
            .collect();
        let on_events: Vec<usize> = fired(&mut rule(), &events).iter().map(|i| i / 2).collect();
        assert_eq!(on_events, expected);
    }

    #[test]
    fn reset_keeps_the_regime() {
        let trades = trades(&calm_burst_calm());
        let expected = fired(&mut rule(), &trades);
        let mut r = rule();
        let mut with_resets = Vec::new();
        for (i, t) in trades.iter().enumerate() {
            if r.should_trigger(t) {
                with_resets.push(i);
            }
            AggregationRule::<Trade>::reset(&mut r);
        }
        assert_eq!(with_resets, expected);
    }

    #[test]
    #[should_panic(expected = "time order")]
    fn trades_out_of_order_panic() {
        let mut r = rule();
        let _ = r.should_trigger(&trade(10, 100.0, 1.0));
        let _ = r.should_trigger(&trade(5, 100.0, 1.0));
    }
}
