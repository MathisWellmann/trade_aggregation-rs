use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the ratio of buys vs total trades.
/// `None` for a candle without trades, instead of dividing 0 by 0.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DirectionalTradeRatio {
    num_buys: usize,
    num_trades: usize,
}

impl CandleComponent for DirectionalTradeRatio {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        (self.num_trades > 0).then(|| self.num_buys as f64 / self.num_trades as f64)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.num_buys = 0;
        self.num_trades = 0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for DirectionalTradeRatio {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.num_trades += 1;
        if trade.size() > 0.0 {
            self.num_buys += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_direction_ratio() {
        let mut m = DirectionalTradeRatio::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), Some(0.7));
    }

    #[test]
    fn an_empty_candle_has_no_ratio() {
        let m = DirectionalTradeRatio::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_is_fully_one_sided() {
        let mut m = DirectionalTradeRatio::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 1.0,
            size: 1.0,
        });
        assert_eq!(m.value(), Some(1.0));
    }
}
