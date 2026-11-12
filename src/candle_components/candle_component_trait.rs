/// Each component of a Candle must fullfill this trait
pub trait CandleComponent {
    /// The type of the value of the component.
    type Value;

    /// The current value of the component
    fn value(&self) -> Self::Value;

    /// Start the next candle.
    ///
    /// A component may keep state that spans candles, e.g. the last price,
    /// so that an empty candle shows the previous close
    /// or the first return of a candle starts at the previous close.
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait,
/// for every type of event `E` it accepts.
pub trait CandleComponentUpdate<E> {
    /// Updates the state with the newest event
    fn update(&mut self, event: &E);

    /// The candle closes at `timestamp`, which is at or after the last event,
    /// in the units of the event timestamps: e.g. integrate a value weighted by time up to it.
    /// Called before the candle is emitted, also for a provisional candle. The default does nothing.
    fn close_at(&mut self, _timestamp: i64) {}
}
