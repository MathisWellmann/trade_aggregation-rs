use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the close price
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Close {
    value: f64,
}

impl CandleComponent for Close {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.value
    }

    #[inline(always)]
    fn reset(&mut self) {}
}

impl<E: AsTrade> CandleComponentUpdate<E> for Close {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.value = trade.price()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close() {
        let mut m = Close::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
            assert_eq!(m.value(), t.price);
        }
        assert_eq!(m.value(), crate::candle_components::tests::TRADES[9].price);
    }
}
