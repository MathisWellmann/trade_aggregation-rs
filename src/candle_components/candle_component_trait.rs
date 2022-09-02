use crate::{TakerTrade, Trade};

/// Each component of a Candle must fullfill this trait
pub trait CandleComponent<T: TakerTrade> {
    /// The current value of the component
    // TODO: make output type generic
    fn value(&self) -> f64;

    /// Updates the state with newest trade information
    fn update(&mut self, trade: &T);

    /// Resets the component state to its default
    fn reset(&mut self);
}
