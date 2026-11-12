use crate::{
    welford_online::WelfordOnline, AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade,
};

/// This 'CandleComponent' keeps track of the standard deviation in the trade sizes
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
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
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
