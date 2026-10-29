use crate::TakerTrade;

/// Defines under what conditions one aggregation period is finished
/// Is generic over the type of candle being produced C,
/// as well as the type of input trade T
pub trait AggregationRule<C, T: TakerTrade> {
    /// The main method defining when the aggregation is done
    ///
    /// # Arguments:
    /// trade: The most recent taker trade (tick) information
    /// candle: Some generic Candle, allowing for information driven decision making
    ///
    /// # Returns:
    /// if true, the aggregation period is finished and a Candle can be emitted
    /// else the aggregation needs to continue
    fn should_trigger(&mut self, trade: &T, candle: &C) -> bool;

    /// Forget what the rule accumulated for the current candle,
    /// because a candle was closed by something other than this rule,
    /// e.g. by the other rule of an [`AnyRule`](crate::AnyRule)
    /// or by the timeout of a [`MaxDuration`](crate::MaxDuration).
    ///
    /// Combinators call this on their inner rules; the aggregator never does,
    /// and a rule restarts its own state when it triggers.
    /// The default does nothing, which is right for rules on a fixed grid such as
    /// [`TimeRule`](crate::TimeRule) and for rules whose state follows the price path
    /// rather than the candle.
    fn reset(&mut self) {}
}
