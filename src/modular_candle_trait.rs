use crate::TakerTrade;

/// A modular candle that can be composed of multiple components
/// Is generic over the type of trade it accepts during the update step,
/// as long as it implements the `TakerTrade` trait
pub trait ModularCandle<T: TakerTrade>: Clone {
    /// Updates the candle information with trade information
    fn update(&mut self, trade: &T);

    /// Resets the state of the candle
    fn reset(&mut self);
}
