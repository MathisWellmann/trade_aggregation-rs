use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the ratio of buy volume vs total volume
#[derive(Clone, Debug, Default)]
pub struct DirectionalVolumeRatio {
    volume: f64,
    buy_volume: f64,
}

impl CandleComponent for DirectionalVolumeRatio {
    fn value(&self) -> f64 {
        self.buy_volume / self.volume
    }

    fn update(&mut self, trade: &Trade) {
        self.volume += trade.size.abs();
        if trade.size > 0.0 {
            self.buy_volume += trade.size;
        }
    }

    fn reset(&mut self) {
        self.volume = 0.0;
        self.buy_volume = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use round::round;

    #[test]
    fn volume_direction_ratio() {
        let mut m = DirectionalVolumeRatio::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(round(m.value(), 4), 0.7143);
    }
}
