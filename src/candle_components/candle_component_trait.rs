use crate::TakerTrade;

/// Each component of a Candle must fullfill this trait
pub trait CandleComponent {
    /// An associated type which is the output type of the value() method
    type Output;
    /// The current value of the component
    // TODO: make output type generic
    fn value(&self) -> Self::Output;

    /// Resets the component state to its default
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait
pub trait CandleComponentUpdate<T: TakerTrade> {
    /// Updates the state with newest trade information
    fn update(&mut self, trade: &T);
}
