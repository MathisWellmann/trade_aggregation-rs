use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the ratio of buys vs total trades
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DirectionalTradeRatio {
    num_buys: usize,
    num_trades: usize,
}

impl CandleComponent for DirectionalTradeRatio {
    type Output = f64;
    #[inline(always)]
    fn value(&self) -> f64 {
        self.num_buys as f64 / self.num_trades as f64
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.num_buys = 0;
        self.num_trades = 0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for DirectionalTradeRatio {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
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
        assert_eq!(m.value(), 0.7);
    }
}
