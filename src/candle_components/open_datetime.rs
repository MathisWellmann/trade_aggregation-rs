use chrono::{DateTime, Utc};

use crate::{CandleComponent, CandleComponentUpdate, TimestampResolution, Timestamped};

/// This 'CandleComponent' keeps track of the opening [`DateTime<Utc>`] of a Candle:
/// the time of its first event. `None` before the first event,
/// or if the timestamp is outside of the range of [`DateTime`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenDateTime {
    init: bool,
    value: Option<DateTime<Utc>>,
}

impl Default for OpenDateTime {
    fn default() -> Self {
        Self {
            init: true,
            value: None,
        }
    }
}

impl CandleComponent for OpenDateTime {
    type Value = Option<DateTime<Utc>>;

    /// Returns the open time of the candle
    #[inline(always)]
    fn value(&self) -> Option<DateTime<Utc>> {
        self.value
    }
    /// This makes sure the next time "update" is called, the new open value is set
    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
    }
}

impl<T: Timestamped> CandleComponentUpdate<T> for OpenDateTime {
    /// Only update the open time if this module is in init mode
    #[inline(always)]
    fn update(&mut self, event: &T) {
        if self.init {
            let stamp = event.timestamp();
            self.value = match T::TIMESTAMP_RESOLUTION {
                TimestampResolution::Millisecond => DateTime::from_timestamp_millis(stamp),
                TimestampResolution::Microsecond => DateTime::from_timestamp_micros(stamp),
                TimestampResolution::Nanosecond => Some(DateTime::from_timestamp_nanos(stamp)),
            };
            self.init = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn open_datetime() {
        let mut m = OpenDateTime::default();
        assert_eq!(m.value(), None);
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        // The first trade is at 1684677200000 ms.
        let expected = Utc
            .with_ymd_and_hms(2023, 5, 21, 13, 53, 20)
            .single()
            .expect("a valid date");
        assert_eq!(m.value(), Some(expected));
    }
}
