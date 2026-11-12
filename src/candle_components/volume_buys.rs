use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the cumulative buy volume of trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeBuys {
    buy_volume: f64,
}

impl CandleComponent for VolumeBuys {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.buy_volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.buy_volume = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for VolumeBuys {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if trade.size() > 0.0 {
            self.buy_volume += trade.size().abs()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Trade;

    #[test]
    fn sums_one_side_and_ignores_trades_without_size() {
        let mut m = VolumeBuys::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 100.0);
        for size in [0.0, -0.0] {
            m.update(&Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert_eq!(m.value(), 100.0);
    }
}
