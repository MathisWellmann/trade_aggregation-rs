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
    fn value(&self) -> f64 {
        self.welford.std_dev()
    }

    fn update(&mut self, trade: &Trade) {
        self.welford.add(trade.price);
    }

    fn reset(&mut self) {
        self.welford = WelfordOnline::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_dev_prices() {
        todo!("Test needed")
    }
}
