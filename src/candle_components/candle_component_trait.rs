use crate::TakerTrade;

/// Each component of a Candle must fullfill this trait
pub trait CandleComponent<T> {
    /// An associated type which is the output type of the value() method
    /// The current value of the component
    fn value(&self) -> T;

    /// Resets the component state to its default
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait
pub trait CandleComponentUpdate<T: TakerTrade> {
    /// Updates the state with newest trade information
    fn update(&mut self, trade: &T);
}
