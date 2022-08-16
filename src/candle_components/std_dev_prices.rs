use crate::welford_online::WelfordOnline;
use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the standard deviation in trade prices
#[derive(Debug, Clone)]
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

impl CandleComponent for StdDevPrices {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.welford.std_dev()
    }

    #[inline(always)]
    fn update(&mut self, trade: &Trade) {
        self.welford.add(trade.price);
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.welford.reset();
    }
}
