/// How a [`Bar`] came about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BarKind {
    /// The rule triggered on an event, which closed the candle.
    Rule,
    /// The [deadline](crate::AggregationRule::deadline) of the rule passed, e.g. the end of a period.
    /// A candle without events is empty, e.g. flat at the previous close
    /// (see [`Gaps::Flat`](crate::Gaps::Flat)).
    Deadline,
    /// [`GenericAggregator::force_close`](crate::GenericAggregator::force_close) closed the candle
    /// at a time of the caller's choosing, whatever the rule says, e.g. a market clock that
    /// samples every symbol of a cross-section at the same instant.
    Forced,
    /// A snapshot of the unfinished candle, which may still change.
    Provisional,
}

/// A candle with the information of when and how it was produced.
///
/// The timestamps are event time, in the units of the events.
/// [`Self::close_ts`] is the earliest time at which the candle is known,
/// so a consumer that joins candles by time must join on it to avoid lookahead.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[must_use]
pub struct Bar<C> {
    candle: C,
    seq: u64,
    open_ts: i64,
    close_ts: i64,
    num_events: u64,
    kind: BarKind,
}

impl<C> Bar<C> {
    pub(crate) fn new(
        candle: C,
        seq: u64,
        (open_ts, close_ts): (i64, i64),
        num_events: u64,
        kind: BarKind,
    ) -> Self {
        assert!(
            open_ts <= close_ts,
            "a candle cannot close ({close_ts}) before it opened ({open_ts})"
        );
        Self {
            candle,
            seq,
            open_ts,
            close_ts,
            num_events,
            kind,
        }
    }

    /// The candle.
    pub fn candle(&self) -> &C {
        &self.candle
    }

    /// The candle, without the information around it.
    pub fn into_candle(self) -> C {
        self.candle
    }

    /// The number of candles the aggregator closed before this one, starting at 0
    /// or at [`GenericAggregator::with_next_seq`](crate::GenericAggregator::with_next_seq).
    /// It counts from the start of the aggregator, so a replay that starts elsewhere numbers
    /// the same candles differently: restore a saved aggregator to keep the numbers.
    ///
    /// A provisional bar has the number the candle gets when it closes,
    /// so a consumer can replace provisional bars by the closed one with the same number.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// When the candle opened: when the previous candle closed, or the first event.
    /// After a gap without events and without empty candles ([`Gaps::Skip`](crate::Gaps::Skip)),
    /// this includes the gap.
    pub fn open_ts(&self) -> i64 {
        self.open_ts
    }

    /// When the candle closed and became known: the event that triggered the rule,
    /// the deadline of the rule, or for a provisional bar the time of the snapshot.
    pub fn close_ts(&self) -> i64 {
        self.close_ts
    }

    /// `close_ts - open_ts`: how long the candle took to fill.
    pub fn duration(&self) -> i64 {
        self.close_ts - self.open_ts
    }

    /// The number of events the candle was updated with.
    /// With [`TriggerTrade::Both`](crate::TriggerTrade::Both) the triggering event counts in both candles.
    pub fn num_events(&self) -> u64 {
        self.num_events
    }

    /// How the bar came about.
    pub fn kind(&self) -> BarKind {
        self.kind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_is_close_minus_open() {
        let bar = Bar::new((), 3, (10, 25), 4, BarKind::Rule);
        assert_eq!(bar.duration(), 15);
        assert_eq!(bar.seq(), 3);
        assert_eq!(bar.num_events(), 4);
        assert_eq!(bar.kind(), BarKind::Rule);
    }

    #[test]
    #[should_panic(expected = "before it opened")]
    fn a_bar_cannot_close_before_it_opened() {
        let _ = Bar::new((), 0, (10, 9), 1, BarKind::Rule);
    }
}
