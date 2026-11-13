use crate::{
    welford_online::WelfordOnline, AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade,
};

/// The coefficient of variation of the trade sizes: their standard deviation divided by
/// their mean, in the base currency, e.g. BTC of BTCUSD (like
/// [`LargestTrade`](super::LargestTrade) and [`BaseVolume`](super::BaseVolume)).
/// Unlike [`StdDevSizes`](super::StdDevSizes), this is scale-free (unitless), so it is
/// comparable across symbols with different typical trade sizes.
///
/// Measures the absolute base size of every trade, not the signed
/// [`TakerTrade::size`], because the mean of a signed quantity whose sign marks a buy or a
/// sell is often close to 0 for a balanced candle, which would make the ratio blow up or
/// flip sign for no reason related to the actual dispersion of trade sizes.
///
/// `None` with fewer than 2 trades (an undefined standard deviation), or if every trade had
/// a zero size (mean size 0, which would otherwise divide 0 by 0).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RelativeStdDevSizes {
    welford: WelfordOnline,
}

impl Default for RelativeStdDevSizes {
    fn default() -> Self {
        Self {
            welford: WelfordOnline::new(),
        }
    }
}

impl CandleComponent for RelativeStdDevSizes {
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

impl<E: AsTrade> CandleComponentUpdate<E> for RelativeStdDevSizes {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.welford.add(trade.base_size().abs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_candle_has_no_relative_std_dev() {
        let m = RelativeStdDevSizes::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_trade_has_no_spread_to_measure() {
        let mut m = RelativeStdDevSizes::default();
        m.update(&crate::Trade {
            timestamp: 0,
            price: 1.0,
            size: 10.0,
        });
        assert_eq!(m.value(), None);
    }

    #[test]
    fn trades_of_zero_size_have_no_mean_to_divide_by() {
        let mut m = RelativeStdDevSizes::default();
        for size in [0.0, -0.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert_eq!(m.value(), None, "zero size everywhere is still 0 / 0");
    }

    #[test]
    fn a_balanced_candle_still_has_a_defined_positive_ratio() {
        // Signed sizes nearly cancel out, but the absolute base size does not.
        let mut m = RelativeStdDevSizes::default();
        for size in [10.0, -10.0, 1.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: 1.0,
                size,
            });
        }
        assert!(m.value().expect("3 trades, not all zero size") > 0.0);
    }

    #[test]
    fn is_scale_free_unlike_the_raw_std_dev() {
        let small = [1.0, 2.0, 3.0, 4.0];
        let large: Vec<f64> = small.iter().map(|s| s * 1_000.0).collect();
        let relative_std_dev = |sizes: &[f64]| {
            let mut m = RelativeStdDevSizes::default();
            for &size in sizes {
                m.update(&crate::Trade {
                    timestamp: 0,
                    price: 1.0,
                    size,
                });
            }
            m.value().expect("4 trades")
        };
        let (a, b) = (relative_std_dev(&small), relative_std_dev(&large));
        assert!((a - b).abs() < 1e-9, "{a} should equal {b}");
    }
}
