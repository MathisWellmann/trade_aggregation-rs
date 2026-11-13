use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the ratio of buy volume vs total volume.
/// `None` for a candle without trades, or if every trade had a zero size,
/// instead of dividing 0 by 0.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DirectionalVolumeRatio {
    volume: f64,
    buy_volume: f64,
}

impl CandleComponent for DirectionalVolumeRatio {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        (self.volume > 0.0).then(|| self.buy_volume / self.volume)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.volume = 0.0;
        self.buy_volume = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for DirectionalVolumeRatio {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
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
        assert_eq!(round(m.value().expect("TRADES is not empty"), 4), 0.7143);
    }

    #[test]
    fn an_empty_candle_has_no_ratio() {
        let m = DirectionalVolumeRatio::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn trades_of_zero_size_carry_no_weight() {
        let mut m = DirectionalVolumeRatio::default();
        for size in [0.0, -0.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert_eq!(m.value(), None, "zero volume everywhere is still 0 / 0");
    }
}
