/// Each component of a Candle must fullfill this trait
pub trait CandleComponent<T> {
    /// An associated type which is the output type of the value() method
    /// The current value of the component
    fn value(&self) -> T;

    /// Resets the component state to its default
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait,
/// for every type of event `E` it accepts.
pub trait CandleComponentUpdate<E> {
    /// Updates the state with the newest event
    fn update(&mut self, event: &E);
}
