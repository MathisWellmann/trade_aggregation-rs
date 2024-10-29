use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the cumulative volume of trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeSells {
    sell_volume: f64,
}

impl CandleComponent<f64> for VolumeSells {
    #[inline(always)]
    fn value(&self) -> f64 {
        self.sell_volume
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.sell_volume = 0.0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for VolumeSells {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        if trade.size().is_sign_negative() {
            self.sell_volume += trade.size().abs()
        }
    }
}
