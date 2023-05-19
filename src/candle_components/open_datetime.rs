use chrono::{DateTime, TimeZone, Utc};

use crate::{CandleComponent, CandleComponentUpdate, TakerTrade, TimestampResolution};

/// This 'CandleComponent' keeps track of the opening [DateTime<Utc>] of a Candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenDateTime {
    init: bool,
    value: DateTime<Utc>,
}

impl Default for OpenDateTime {
    fn default() -> Self {
        Self {
            init: true,
            value: Default::default(),
        }
    }
}

impl CandleComponent<DateTime<Utc>> for OpenDateTime {
    /// Returns the open price of the candle
    #[inline(always)]
    fn value(&self) -> DateTime<Utc> {
        self.value
    }
    /// This makes sure the next time "update" is called, the new open value is set
    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for OpenDateTime {
    /// Only update the open price if this module is in init mode
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            let stamp = trade.timestamp();
            self.value = match trade.timestamp_resolution() {
                TimestampResolution::Second => Utc.timestamp(stamp, 0),
                TimestampResolution::Millisecond => Utc.timestamp_millis(trade.timestamp()),
                TimestampResolution::Microsecond => Utc.timestamp_nanos(stamp * 1000),
                TimestampResolution::Nanosecond => Utc.timestamp_nanos(stamp),
            };
            self.init = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_datetime() {
        let mut m = OpenDateTime::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), Utc.ymd(1985, 11, 5).and_hms(0, 53, 20));
    }
}
