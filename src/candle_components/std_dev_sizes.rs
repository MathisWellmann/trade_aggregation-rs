use crate::{
    welford_online::WelfordOnline, AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade,
};

/// This 'CandleComponent' keeps track of the standard deviation in the trade sizes.
/// `None` with fewer than 2 trades, instead of an arbitrary fallback for an undefined spread.
///
/// This is in the raw base-currency units of the symbol, so it is not comparable across
/// symbols with different typical trade sizes. Use
/// [`RelativeStdDevSizes`](super::RelativeStdDevSizes) for a scale-free (unitless)
/// alternative.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StdDevSizes {
    welford: WelfordOnline,
}

impl Default for StdDevSizes {
    fn default() -> Self {
        Self {
            welford: WelfordOnline::new(),
        }
    }
}
impl CandleComponent for StdDevSizes {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.welford.std_dev()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.welford.reset();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for StdDevSizes {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.welford.add(trade.size());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_candle_has_no_std_dev() {
        let m = StdDevSizes::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_has_no_spread_to_measure() {
        let mut m = StdDevSizes::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 1.0,
            size: 10.0,
        });
        assert_eq!(m.value(), None);
    }

    #[test]
    fn two_trades_have_a_defined_std_dev() {
        let mut m = StdDevSizes::default();
        for size in [10.0, -20.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert!(m.value().expect("2 trades") > 0.0);
        m.reset();
        assert_eq!(m.value(), None);
    }
}
