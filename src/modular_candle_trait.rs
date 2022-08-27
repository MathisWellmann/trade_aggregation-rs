/// A modular candle that can be composed of multiple components
pub trait ModularCandle: Clone + Default {
    /// Associated type for the input of the update method, should be bound
    /// to TakerTrade so should probably just make this generic to the trait
    /// and bound it.
    type TradeType;
    /// Updates the candle information with trade information
    fn update(&mut self, trade: &Self::TradeType);

    /// Resets the state of the candle
    fn reset(&mut self);
}
