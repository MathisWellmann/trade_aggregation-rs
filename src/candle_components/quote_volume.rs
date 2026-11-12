use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// The traded volume in the quote currency (the notional), e.g. USD of BTCUSD,
/// whatever the [`TakerTrade::SIZE_UNIT`] of the trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuoteVolume {
    volume: f64,
}

impl CandleComponent for QuoteVolume {
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

impl<T: TakerTrade> CandleComponentUpdate<T> for QuoteVolume {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.volume += trade.quote_size().abs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_volume_sums_the_quote_sizes() {
        let mut m = QuoteVolume::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 140.0);
        m.reset();
        assert_eq!(m.value(), 0.0);
    }
}
