use crate::{CandleComponent, CandleComponentUpdate, Timestamped};

/// Measures the velocity of candle creation based on the formula:
/// 1.0 / t  , where t is measured in seconds and at least one second
/// The higher the velocity the faster the candle has been created
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeVelocity {
    init: bool,
    /// The timestamp of the first event, in event units.
    init_time: i64,
    /// The seconds from the first to the last event.
    elapsed_s: f64,
}

impl Default for TimeVelocity {
    fn default() -> Self {
        Self {
            init: true,
            init_time: 0,
            elapsed_s: 0.0,
        }
    }
}

impl CandleComponent for TimeVelocity {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        // cap elapsed_s to avoid time_velocity being infinite
        1.0 / self.elapsed_s.max(1.0)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.init = true;
        self.elapsed_s = 0.0;
    }
}

impl<T: Timestamped> CandleComponentUpdate<T> for TimeVelocity {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if self.init {
            self.init_time = trade.timestamp();
            self.init = false;
        }
        let units_per_s = T::TIMESTAMP_RESOLUTION.units_per_millisecond() as f64 * 1_000.0;
        self.elapsed_s = (trade.timestamp() - self.init_time) as f64 / units_per_s;
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

    #[test]
    fn time_velocity_counts_fractions_of_a_second() {
        let mut comp = TimeVelocity::default();
        for timestamp in [0, 2_500] {
            comp.update(&crate::Trade {
                timestamp,
                price: 1.0,
                size: 1.0,
            });
        }
        assert_eq!(comp.value(), 0.4);
        comp.reset();
        assert_eq!(comp.value(), 1.0, "an empty candle is capped at one second");
    }
}
