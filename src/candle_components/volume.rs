use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the cumulative volume of trades,
/// in the unit of [`TakerTrade::size`]; see also [`BaseVolume`](super::BaseVolume)
/// and [`QuoteVolume`](super::QuoteVolume).
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Volume {
    volume: f64,
}

impl CandleComponent for Volume {
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

impl<E: AsTrade> CandleComponentUpdate<E> for Volume {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.volume += trade.size().abs()
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
