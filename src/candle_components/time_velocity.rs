use crate::{CandleComponent, CandleComponentUpdate, TakerTrade, TimestampResolution};

/// Measures the velocity of candle creation based on the formula:
/// 1.0 / t  , where t is measured in seconds
/// The higher the velocity the faster the candle has been created
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeVelocity {
    init: bool,
    // unix time (in seconds)
    init_time: i64,
    last_time: i64,
}

impl Default for TimeVelocity {
    fn default() -> Self {
        Self {
            init: true,
            init_time: 0,
            last_time: 0,
        }
    }
}

impl CandleComponent<f64> for TimeVelocity {
    #[inline(always)]
    fn value(&self) -> f64 {
        let mut elapsed_s: f64 = (self.last_time - self.init_time) as f64;
        if elapsed_s < 1.0 {
            // cap elapsed_s to avoid time_velocity being infinite
            elapsed_s = 1.0;
        }
        1.0 / elapsed_s
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.init = true
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for TimeVelocity {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        let div = match trade.timestamp_resolution() {
            TimestampResolution::Second => 1,
            TimestampResolution::Millisecond => 1_000,
            TimestampResolution::Microsecond => 1_000_000,
            TimestampResolution::Nanosecond => 1_000_000_000,
        };
        if self.init {
            self.init_time = trade.timestamp() / div;
            self.init = false;
        }
        self.last_time = trade.timestamp() / div;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::candle_components::tests::TRADES;

    #[test]
    fn time_velocity() {
        let mut comp = TimeVelocity::default();
        for t in TRADES.iter() {
            comp.update(t);
        }
        assert_eq!(round::round(comp.value(), 3), 0.011);
    }
}
