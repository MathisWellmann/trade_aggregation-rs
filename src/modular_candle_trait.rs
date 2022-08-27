use crate::TakerTrade;

/// A modular candle that can be composed of multiple components
pub trait ModularCandle<T: TakerTrade>: Clone + Default {
    /// Associated type for the input of the update method, should be bound
    /// to TakerTrade so should probably just make this generic to the trait
    /// and bound it.
    /// Updates the candle information with trade information
    fn update(&mut self, trade: &T);

    /// Resets the state of the candle
    fn reset(&mut self);
}
