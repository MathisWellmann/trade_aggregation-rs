use crate::{
    welford_online::WelfordOnline, AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade,
};

/// The coefficient of variation of the trade prices: their standard deviation divided by
/// their mean. Unlike [`StdDevPrices`](super::StdDevPrices), this is scale-free (unitless),
/// so it is comparable across symbols with different price levels, e.g. when pooling many
/// symbols into one downstream model.
///
/// `None` with fewer than 2 trades (an undefined standard deviation), or if the mean price
/// is exactly 0 (which trade prices should never be, but this guards the division anyway).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelativeStdDevPrices {
    welford: WelfordOnline,
}

impl Default for RelativeStdDevPrices {
    fn default() -> Self {
        Self {
            welford: WelfordOnline::new(),
        }
    }
}

impl CandleComponent for RelativeStdDevPrices {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        let std_dev = self.welford.std_dev()?;
        let mean = self.welford.mean();
        (mean != 0.0).then_some(std_dev / mean)
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.welford.reset();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for RelativeStdDevPrices {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.welford.add(trade.price());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_candle_has_no_relative_std_dev() {
        let m = RelativeStdDevPrices::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_has_no_spread_to_measure() {
        let mut m = RelativeStdDevPrices::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 100.0,
            size: 1.0,
        });
        assert_eq!(m.value(), None);
    }

    #[test]
    fn is_scale_free_unlike_the_raw_std_dev() {
        // Two symbols with the same relative spread, but a price 100x apart.
        let cheap = [1.0, 1.1, 0.9, 1.0];
        let expensive: Vec<f64> = cheap.iter().map(|p| p * 100.0).collect();
        let relative_std_dev = |prices: &[f64]| {
            let mut m = RelativeStdDevPrices::default();
            for &price in prices {
                m.update(&crate::Trade {
                    timestamp: 0,
                    price,
                    size: 1.0,
                });
            }
            m.value().expect("4 trades")
        };
        let (a, b) = (relative_std_dev(&cheap), relative_std_dev(&expensive));
        assert!((a - b).abs() < 1e-9, "{a} should equal {b}");
    }

    #[test]
    fn reset_forgets_the_trades() {
        let mut m = RelativeStdDevPrices::default();
        for price in [100.0, 102.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price,
                size: 1.0,
            });
        }
        assert!(m.value().is_some());
        m.reset();
        assert_eq!(m.value(), None);
    }
}
