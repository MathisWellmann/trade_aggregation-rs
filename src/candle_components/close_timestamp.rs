use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the closing timestamp of a Candle, using the
/// same unit resolution as the underlying input of [`TakerTrade.timestamp()`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CloseTimeStamp<T> {
    value: T,
}

impl<T: Default> Default for CloseTimeStamp<T> {
    fn default() -> Self {
        Self {
            value: Default::default(),
        }
    }
}

impl CandleComponent<i64> for CloseTimeStamp<i64> {
    #[inline(always)]
    fn value(&self) -> i64 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

impl<T: TakerTrade> CandleComponentUpdate<T> for CloseTimeStamp<i64> {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.value = trade.timestamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_timestamp() {
        let mut m = CloseTimeStamp::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 1684677290_000);
    }
}
