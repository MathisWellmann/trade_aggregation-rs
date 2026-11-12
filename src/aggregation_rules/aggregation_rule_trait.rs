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

    /// The time at which the rule closes the current candle without another event, if any,
    /// in the units of the event timestamps.
    ///
    /// The aggregator closes the candle at this time as soon as an event or a watermark
    /// ([`GenericAggregator::advance_to`](crate::GenericAggregator::advance_to)) reaches it,
    /// and stamps the candle with it. An event at the deadline belongs to the next candle.
    /// The default is `None`: the rule closes candles only on events.
    fn deadline(&self) -> Option<i64> {
        None
    }

    /// The aggregator closed the candle at [`Self::deadline`]: start the next candle.
    ///
    /// A rule on a clock moves on by one period, so that the next deadline is later.
    /// The default forgets the current candle with [`Self::reset`].
    fn on_deadline(&mut self) {
        self.reset();
    }

    /// No events arrived up to `timestamp`, which is at or after [`Self::deadline`]:
    /// skip the candles that would close empty, so that the deadline moves past `timestamp`
    /// (or to `None`).
    ///
    /// The aggregator calls this instead of [`Self::on_deadline`] for each empty period,
    /// so that a long gap costs one call. The default does nothing.
    fn skip_to(&mut self, _timestamp: i64) {}
}
