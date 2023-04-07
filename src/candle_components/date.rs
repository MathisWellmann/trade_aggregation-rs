use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the opening price of a Candle
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeStamp {
    init: bool,
    value: i64,
}

impl CandleComponent for TimeStamp {
    type Output = i64;
    /// Returns the open price of the candle
    #[inline(always)]
    fn value(&self) -> i64 {
        self.value
    }
    /// This makes sure the next time "update" is called, the new open value is set
    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for TimeStamp {
    /// Only update the open price if this module is in init mode
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            self.value = trade.timestamp();
            self.init = false;
        }
    }
}
