use crate::welford_online::WelfordOnline;
use crate::{CandleComponent, Trade};

/// This 'CandleComponent' keeps track of the standard deviation in the trade sizes
#[derive(Debug, Clone)]
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
    fn value(&self) -> f64 {
        self.welford.std_dev()
    }

    fn update(&mut self, trade: &Trade) {
        self.welford.add(trade.size);
    }

    fn reset(&mut self) {
        self.welford.reset();
    }
}
