/// Defines under what conditions one aggregation period is finished.
/// Is generic over the type of input event `E`, e.g. a [`TakerTrade`](crate::TakerTrade).
///
/// A rule does not see the candle: it keeps whatever state it needs itself,
/// so the same rule works with any candle type.
pub trait AggregationRule<E> {
    /// The main method defining when the aggregation is done
    ///
    /// # Arguments:
    /// event: The most recent event, e.g. a taker trade (tick)
    ///
    /// # Returns:
    /// if true, the aggregation period is finished and a Candle can be emitted
    /// else the aggregation needs to continue
    fn should_trigger(&mut self, event: &E) -> bool;

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
