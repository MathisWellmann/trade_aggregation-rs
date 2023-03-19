use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the ratio of buy volume vs total volume
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DirectionalVolumeRatio {
    volume: f64,
    buy_volume: f64,
}

impl CandleComponent for DirectionalVolumeRatio {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.buy_volume / self.volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.volume = 0.0;
        self.buy_volume = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for DirectionalVolumeRatio {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.volume += trade.size().abs();
        if trade.size() > 0.0 {
            self.buy_volume += trade.size();
        }
    }
}

#[cfg(test)]
mod tests {
    use round::round;

    use super::*;

    #[test]
    fn volume_direction_ratio() {
        let mut m = DirectionalVolumeRatio::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(round(m.value(), 4), 0.7143);
    }
}
