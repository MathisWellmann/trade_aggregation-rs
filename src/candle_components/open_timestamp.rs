use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the opening timestamp of a Candle, using the
/// same unit resolution as the underlying input of [TakerTrade.timestamp()].
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenTimeStamp<T> {
    init: bool,
    value: T,
}

impl CandleComponent<i64> for OpenTimeStamp<i64> {
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

impl<T: TakerTrade> CandleComponentUpdate<T> for OpenTimeStamp<i64> {
    /// Only update the open price if this module is in init mode
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            self.value = trade.timestamp();
            self.init = false;
        }
    }
}
