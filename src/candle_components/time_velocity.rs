use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// Measures the velocity of candle creation based on the formula:
/// 1.0 / t  , where t is measured in minutes
/// The higher the velocity the faster the candle has been created
/// Assumes trade timestamps in milliseconds
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeVelocity {
    init: bool,
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

impl CandleComponent for TimeVelocity {
    type Output = f64;
    #[inline(always)]
    fn value(&self) -> f64 {
        let mut elapsed_s: f64 = (self.last_time - self.init_time) as f64 / 1000.0;
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
        if self.init {
            self.init_time = trade.timestamp();
        }
        self.last_time = trade.timestamp();
    }
}
