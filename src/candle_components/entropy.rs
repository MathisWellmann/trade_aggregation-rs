use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// A `CandleComponent` that computes the binary entropy of whether a trade is a buy or a sell.
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entropy {
    buys: usize,
    total_observed_trades: usize,
}

impl CandleComponent for Entropy {
    type Value = f64;

    fn value(&self) -> f64 {
        let pt = self.buys as f64 / self.total_observed_trades as f64;
        let pn = 1_f64 - pt;

        let mut h = pt * pt.log2() + pn * pn.log2();
        if h.is_nan() {
            h = 0.0;
        }

        -h
    }

    fn reset(&mut self) {
        self.buys = 0;
        self.total_observed_trades = 0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for Entropy {
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if trade.size() > 0.0 {
            self.buys += 1
        }
        self.total_observed_trades += 1;
    }
}
