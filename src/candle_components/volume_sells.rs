use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the cumulative sell volume of trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeSells {
    sell_volume: f64,
}

impl CandleComponent for VolumeSells {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.sell_volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.sell_volume = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for VolumeSells {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if trade.size() < 0.0 {
            self.sell_volume += trade.size().abs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Trade;

    #[test]
    fn sums_one_side_and_ignores_trades_without_size() {
        let mut m = VolumeSells::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 40.0);
        for size in [0.0, -0.0] {
            m.update(&Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert_eq!(m.value(), 40.0);
    }
}
