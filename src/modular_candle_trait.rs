use crate::Trade;

/// A modular candle that can be composed of multiple components
pub trait ModularCandle: Clone + Default {
    /// Updates the candle information with trade information
    fn update(&mut self, trade: &Trade);

    /// Resets the state of the candle
    fn reset(&mut self);
}
