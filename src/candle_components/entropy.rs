use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// A `CandleComponent` that computes the binary entropy of whether a trade is a buy or a sell.
/// `None` for a candle without trades: the distribution of buy vs. sell is undefined without
/// an observation, instead of forcing a 0.0/0.0 division to 0.
#[derive(Default, Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entropy {
    buys: usize,
    total_observed_trades: usize,
}

/// `p * log2(p)`, with the Shannon convention `0 * log2(0) = 0`
/// instead of the `NaN` that `0.0 * f64::NEG_INFINITY` would otherwise give.
#[inline(always)]
fn p_log2_p(p: f64) -> f64 {
    if p <= 0.0 {
        0.0
    } else {
        p * p.log2()
    }
}

impl CandleComponent for Entropy {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        if self.total_observed_trades == 0 {
            return None;
        }
        let pt = self.buys as f64 / self.total_observed_trades as f64;
        let pn = 1.0 - pt;
        Some(-(p_log2_p(pt) + p_log2_p(pn)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_candle_has_no_entropy() {
        let m = Entropy::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_is_a_degenerate_distribution() {
        let mut m = Entropy::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 1.0,
            size: 1.0,
        });
        assert_eq!(m.value(), Some(0.0));
    }

    #[test]
    fn an_even_split_has_maximal_entropy() {
        let mut m = Entropy::default();
        for size in [1.0, -1.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert_eq!(m.value(), Some(1.0));
        m.reset();
        assert_eq!(m.value(), None);
    }
}
