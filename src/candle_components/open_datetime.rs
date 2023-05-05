use crate::CandleComponent;
use crate::CandleComponentUpdate;
use crate::TakerTrade;
use crate::TimestampResolution;
use chrono::{DateTime, TimeZone, Utc};

/// This 'CandleComponent' keeps track of the opening [DateTime<Utc>] of a Candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenDateTime {
    init: bool,
    value: DateTime<Utc>,
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
