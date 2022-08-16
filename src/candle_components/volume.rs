use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the cumulative volume
#[derive(Debug, Default, Clone)]
pub struct Volume {
    volume: f64,
}

impl CandleComponent for Volume {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.volume
    }

    #[inline(always)]
    fn update(&mut self, trade: &Trade) {
        self.volume += trade.size.abs()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.volume = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume() {
        let mut m = Volume::default();
        let mut sum: f64 = 0.0;
        for t in &crate::candle_components::tests::TRADES {
            sum += t.size.abs();
            m.update(t);
            assert_eq!(m.value(), sum);
        }
    }
}
