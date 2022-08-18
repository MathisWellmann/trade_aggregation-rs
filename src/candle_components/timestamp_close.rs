use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the timestamp at the candle close
#[derive(Default, Debug, Clone)]
pub struct TimestampClose {
    value: i64,
}

impl CandleComponent<i64> for TimestampClose {
    #[inline(always)]
    fn value(&self) -> i64 {
        self.value
    }

    #[inline(always)]
    fn update(&mut self, trade: &Trade) {
        self.value = trade.timestamp;
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_close() {
        let mut m = TimestampClose::default();

        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), t.timestamp);
        }
    }
}
