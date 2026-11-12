/// A modular candle that can be composed of multiple components
/// Is generic over the type of event it accepts during the update step,
/// e.g. a type that implements the [`TakerTrade`](crate::TakerTrade) trait.
pub trait ModularCandle<E>: Clone + Default {
    /// Updates the candle information with the event information
    fn update(&mut self, event: &E);

    /// Resets the state of the candle
    fn reset(&mut self);

    /// The candle closes at `timestamp`, see [`CandleComponentUpdate::close_at`](crate::CandleComponentUpdate::close_at).
    /// The default does nothing.
    fn close_at(&mut self, _timestamp: i64) {}
}
