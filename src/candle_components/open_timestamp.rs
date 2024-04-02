use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the opening timestamp of a Candle, using the
/// same unit resolution as the underlying input of [`TakerTrade.timestamp()`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpenTimeStamp<T> {
    init: bool,
    value: T,
}

impl<T: Default> Default for OpenTimeStamp<T> {
    fn default() -> Self {
        Self {
            init: true,
            value: Default::default(),
        }
    }
}

impl CandleComponent<i64> for OpenTimeStamp<i64> {
    #[inline(always)]
    fn value(&self) -> i64 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for OpenTimeStamp<i64> {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            self.value = trade.timestamp();
            self.init = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_timestamp() {
        let mut m = OpenTimeStamp::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 1684677200000);
    }
}
