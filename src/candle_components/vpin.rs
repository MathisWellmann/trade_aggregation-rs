use super::{CandleComponent, CandleComponentUpdate};
use crate::TakerTrade;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vpin {}

impl CandleComponent<f64> for Vpin {
    fn value(&self) -> f64 {
        todo!()
    }

    fn reset(&mut self) {
        todo!()
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for Vpin {
    #[inline(always)]
    fn update(&mut self, _: &T) {
        todo!()
    }
}
