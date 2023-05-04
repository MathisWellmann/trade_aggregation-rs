use chrono::{DateTime, Utc};

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
            self.value = Utc.timestamp_millis(trade.timestamp(), 0);
            self.init = false;
        }
    }
}
