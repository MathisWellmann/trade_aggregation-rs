use crate::TakerTrade;

/// Each component of a Candle must fullfill this trait
pub trait CandleComponent {
    /// The current value of the component
    // TODO: make output type generic
    fn value(&self) -> f64;

    /// Resets the component state to its default
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait
pub trait CandleComponentUpdate<T: TakerTrade> {
    /// Updates the state with newest trade information
    fn update(&mut self, trade: &T);
}
