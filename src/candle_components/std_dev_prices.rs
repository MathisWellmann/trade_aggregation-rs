use crate::{welford_online::WelfordOnline, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the standard deviation in trade prices
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StdDevPrices {
    welford: WelfordOnline,
}

impl Default for StdDevPrices {
    fn default() -> Self {
        Self {
            welford: WelfordOnline::new(),
        }
    }
}

impl CandleComponent<f64> for StdDevPrices {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.welford.std_dev()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.welford.reset();
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for StdDevPrices {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.welford.add(trade.price());
    }
}
