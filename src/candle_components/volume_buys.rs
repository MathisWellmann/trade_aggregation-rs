use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the cumulative buy volume of trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeBuys {
    buy_volume: f64,
}

impl CandleComponent<f64> for VolumeBuys {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.buy_volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.buy_volume = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for VolumeBuys {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if trade.size().is_sign_positive() {
            self.buy_volume += trade.size().abs()
        }
    }
}
