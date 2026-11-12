use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// The traded volume in the base currency, e.g. BTC of BTCUSD,
/// whatever the [`TakerTrade::SIZE_UNIT`] of the trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BaseVolume {
    volume: f64,
}

impl CandleComponent for BaseVolume {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.volume = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for BaseVolume {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.volume += trade.base_size().abs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_volume_divides_quote_sizes_by_the_price() {
        let mut m = BaseVolume::default();
        for t in &crate::candle_components::tests::TRADES[..2] {
            m.update(t);
        }
        // 10 / 100 + 10 / 101
        assert!((m.value() - (0.1 + 10.0 / 101.0)).abs() < 1e-12);
        m.reset();
        assert_eq!(m.value(), 0.0);
    }
}
