//! The periods of the time rules.

use crate::{MillisecondPeriod, TimestampResolution};

/// Where the periods of a [`Clock`] start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) enum Origin {
    /// At `offset` milliseconds after the Unix epoch, and every period from there.
    Epoch {
        /// The offset from the epoch, in milliseconds.
        offset_ms: u64,
    },
    /// At the first event.
    FirstEvent,
}

/// Consecutive periods `[start, start + period)` that tile the time line from an origin.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Clock {
    period: MillisecondPeriod,
    origin: Origin,
    /// The origin in event units, known with the first event.
    origin_units: Option<i64>,
    /// The start of the current period in event units, `None` before the first event.
    start: Option<i64>,
}

impl Clock {
    pub(crate) fn new(period: MillisecondPeriod, origin: Origin) -> Self {
        Self {
            period,
            origin,
            origin_units: None,
            start: None,
        }
    }

    /// The start of the period that contains `timestamp`.
    fn period_start(origin: i64, period: i64, timestamp: i64) -> i64 {
        debug_assert!(period > 0);
        let start = timestamp - (timestamp - origin).rem_euclid(period);
        debug_assert!(start <= timestamp);
        debug_assert!(timestamp - start < period);
        start
    }

    /// The end of the current period, `None` before the first event.
    pub(crate) fn deadline(&self, resolution: TimestampResolution) -> Option<i64> {
        let period = self.period.in_units(resolution);
        self.start.map(|start| start.saturating_add(period))
    }

    /// Start the clock at the first event, or report whether `timestamp` is past the current period.
    /// A timestamp past the period moves the clock to the period that contains it,
    /// skipping the periods in between.
    pub(crate) fn observe(&mut self, timestamp: i64, resolution: TimestampResolution) -> bool {
        let period = self.period.in_units(resolution);
        let origin = *self.origin_units.get_or_insert(match self.origin {
            Origin::Epoch { offset_ms } => i64::try_from(offset_ms)
                .unwrap_or(i64::MAX)
                .saturating_mul(resolution.units_per_millisecond()),
            Origin::FirstEvent => timestamp,
        });
        let Some(start) = self.start else {
            self.start = Some(Self::period_start(origin, period, timestamp));
            return false;
        };
        if timestamp < start.saturating_add(period) {
            return false;
        }
        self.start = Some(Self::period_start(origin, period, timestamp));
        true
    }

    /// Move on to the next period.
    pub(crate) fn advance(&mut self, resolution: TimestampResolution) {
        let period = self.period.in_units(resolution);
        let start = self
            .start
            .expect("the aggregator closes a period only after the clock started");
        self.start = Some(start.saturating_add(period));
    }

    /// Move to the period that contains `timestamp`, if it is later.
    pub(crate) fn skip_to(&mut self, timestamp: i64, resolution: TimestampResolution) {
        let (Some(start), Some(origin)) = (self.start, self.origin_units) else {
            return;
        };
        let period = self.period.in_units(resolution);
        let skipped = Self::period_start(origin, period, timestamp);
        self.start = Some(start.max(skipped));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: TimestampResolution = TimestampResolution::Millisecond;

    #[test]
    fn epoch_periods_include_negative_timestamps() {
        assert_eq!(Clock::period_start(0, 100, -1), -100);
        assert_eq!(Clock::period_start(0, 100, -100), -100);
        assert_eq!(Clock::period_start(0, 100, 99), 0);
        assert_eq!(Clock::period_start(30, 100, 29), -70);
        assert_eq!(Clock::period_start(30, 100, 130), 130);
    }

    #[test]
    fn a_timestamp_past_the_period_jumps_to_its_period() {
        let mut clock = Clock::new(
            MillisecondPeriod::from_non_zero(100),
            Origin::Epoch { offset_ms: 0 },
        );
        assert_eq!(clock.deadline(MS), None);
        assert!(!clock.observe(150, MS));
        assert_eq!(clock.deadline(MS), Some(200));
        assert!(!clock.observe(199, MS));
        assert!(clock.observe(1_050, MS));
        assert_eq!(clock.deadline(MS), Some(1_100));
    }

    #[test]
    fn the_first_event_can_be_the_origin() {
        let mut clock = Clock::new(MillisecondPeriod::from_non_zero(100), Origin::FirstEvent);
        assert!(!clock.observe(150, MS));
        assert_eq!(clock.deadline(MS), Some(250));
        clock.advance(MS);
        assert_eq!(clock.deadline(MS), Some(350));
        clock.skip_to(1_000, MS);
        assert_eq!(clock.deadline(MS), Some(1_050));
        clock.skip_to(10, MS);
        assert_eq!(clock.deadline(MS), Some(1_050), "skipping never moves back");
    }

    #[test]
    fn an_offset_shifts_the_periods() {
        let mut clock = Clock::new(
            MillisecondPeriod::from_non_zero(100),
            Origin::Epoch { offset_ms: 30 },
        );
        assert!(!clock.observe(1_000, TimestampResolution::Microsecond));
        // 100 ms = 100_000 us, the offset 30 ms = 30_000 us.
        assert_eq!(
            clock.deadline(TimestampResolution::Microsecond),
            Some(30_000)
        );
    }
}
