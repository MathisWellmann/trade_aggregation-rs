use crate::{
    price_source::LogReturns, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// Creates price change bars: a candle closes at every `n_changes`-th change of the price,
/// i.e. every `n_changes`-th non-zero log return of the price source.
///
/// With [`MidPrice`](crate::MidPrice) these are mid-change bars, on the event clock of
/// limit order book models such as Cont and de Larrard (2013), which count the moves of the mid
/// price and ignore the quote updates that do not move it (e.g. a change of the size at the best
/// bid). Like a [`TickRule`](crate::TickRule) the rule counts events, but only those that move the
/// price, so a burst of order book updates without a move does not close candles.
/// With [`Microprice`](crate::Microprice), a change of the sizes moves the price and counts.
///
/// The prices are those of the trades by default. Then a trade at the same price as the previous
/// one does not count, but every trade on the other side of the spread does: the bid-ask bounce
/// (Roll 1984) changes the price without a move of the market.
/// Prefer the mid price with [`Self::with_price_source`].
///
/// To calibrate the threshold with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Linear`](crate::Scaling::Linear): the number of changes grows in proportion to the
/// duration.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PriceChangeRule<P = TradePrice> {
    /// A `u32`, so that it converts to and from the `f64` threshold without loss.
    n_changes: u32,
    returns: LogReturns<P>,
    /// The changes in the current candle.
    count: u32,
}

impl PriceChangeRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `n_changes`: The number of price changes after which a candle closes.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `n_changes` is zero.
    pub fn new(n_changes: u32) -> Result<Self> {
        if n_changes == 0 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            n_changes,
            returns: LogReturns::new(TradePrice),
            count: 0,
        })
    }
}

impl<P> PriceChangeRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same number of changes.
    /// The first price of `source` is not a change.
    pub fn with_price_source<Q>(self, source: Q) -> PriceChangeRule<Q> {
        PriceChangeRule {
            n_changes: self.n_changes,
            returns: LogReturns::new(source),
            count: 0,
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for PriceChangeRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return false;
        };
        if r == 0.0 {
            return false;
        }
        // No overflow: `count < n_changes <= u32::MAX` after every event, see below,
        // and a lower threshold set within a candle only lowers `n_changes`.
        self.count += 1;
        let trigger = self.count >= self.n_changes;
        if trigger {
            self.count = 0;
        }
        debug_assert!(self.count < self.n_changes);
        trigger
    }

    /// Restart the count; the last price stays the reference, so a change on the next event
    /// counts in the new candle.
    fn reset(&mut self) {
        self.count = 0;
    }
}

impl<P> ThresholdRule for PriceChangeRule<P> {
    fn threshold(&self) -> f64 {
        f64::from(self.n_changes)
    }

    /// Rounds `threshold` to whole changes, at least one and at most `u32::MAX`.
    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        let changes = threshold.round().clamp(1.0, f64::from(u32::MAX));
        // A whole number in the range of `u32` after the clamp, so the cast is exact.
        self.n_changes = changes as u32;
        debug_assert!(self.n_changes >= 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{
            quote_event, trade_event, trades_at_prices, trigger_indices,
        },
        Microprice, MidPrice, Trade,
    };

    fn rule(n_changes: u32) -> PriceChangeRule {
        PriceChangeRule::new(n_changes).expect("valid number of changes")
    }

    #[test]
    fn new_rejects_zero_changes() {
        assert!(PriceChangeRule::new(0).is_err());
        assert!(PriceChangeRule::new(1).is_ok());
        assert!(PriceChangeRule::new(u32::MAX).is_ok());
    }

    #[test]
    fn counts_only_the_changes() {
        // Changes at trades 2 (to 101), 5 (to 100), 7 (to 102) and 8 (to 103).
        let trades = trades_at_prices(&[
            100.0, 100.0, 101.0, 101.0, 101.0, 100.0, 100.0, 102.0, 103.0,
        ]);
        assert_eq!(trigger_indices(&mut rule(2), &trades), vec![5, 8]);
        assert_eq!(trigger_indices(&mut rule(1), &trades), vec![2, 5, 7, 8]);
    }

    #[test]
    fn the_first_price_is_no_change() {
        let trades = trades_at_prices(&[100.0, 100.0, 100.5]);
        assert_eq!(trigger_indices(&mut rule(1), &trades), vec![2]);
    }

    #[test]
    fn a_flat_price_never_triggers() {
        let trades = trades_at_prices(&[100.0; 1_000]);
        assert!(trigger_indices(&mut rule(1), &trades).is_empty());
    }

    #[test]
    fn reset_restarts_the_count_but_keeps_the_reference_price() {
        let mut r = rule(3);
        assert!(trigger_indices(&mut r, &trades_at_prices(&[100.0, 101.0, 102.0])).is_empty());
        AggregationRule::<Trade>::reset(&mut r);
        // 103 is a change from 102, the last price before the reset; 104 and 105 complete three.
        let trades = trades_at_prices(&[103.0, 103.0, 104.0, 105.0]);
        assert_eq!(trigger_indices(&mut r, &trades), vec![3]);
    }

    #[test]
    fn set_threshold_rounds_to_whole_changes() {
        let mut r = rule(5);
        assert_eq!(r.threshold(), 5.0);
        for (threshold, expected) in [(2.4, 2.0), (2.6, 3.0), (0.3, 1.0), (1e20, 4_294_967_295.0)] {
            r.set_threshold(threshold).expect("valid threshold");
            assert_eq!(r.threshold(), expected, "threshold {threshold}");
        }
        for invalid in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(r.set_threshold(invalid).is_err(), "threshold {invalid}");
        }
        assert_eq!(r.threshold(), 4_294_967_295.0);
    }

    #[test]
    fn a_lower_threshold_within_a_candle_closes_it_at_the_next_change() {
        let mut r = rule(5);
        let trades = trades_at_prices(&[100.0, 101.0, 102.0, 103.0, 103.0, 104.0, 105.0]);
        assert!(trigger_indices(&mut r, &trades[..4]).is_empty());
        r.set_threshold(2.0).expect("valid threshold");
        // Three changes so far: the next change (to 104) closes the candle at once,
        // and 105 is the first change of the next one.
        assert_eq!(trigger_indices(&mut r, &trades[4..]), vec![1]);
    }

    #[test]
    fn mid_change_bars_ignore_updates_that_do_not_move_the_mid() {
        let events = [
            quote_event(99.0, 1.0, 101.0, 1.0), // mid 100, micro 100
            trade_event(99.0),
            quote_event(99.0, 5.0, 101.0, 1.0), // mid 100, micro 100.67
            trade_event(101.0),
            quote_event(99.5, 1.0, 101.0, 1.0), // mid 100.25, micro 100.25
            quote_event(99.5, 2.0, 101.0, 3.0), // mid 100.25, micro 100.1
            trade_event(99.5),
            quote_event(99.0, 1.0, 101.0, 1.0), // mid 100, micro 100
            quote_event(99.0, 1.0, 101.0, 2.0), // mid 100, micro 99.67
            quote_event(100.0, 1.0, 102.0, 1.0), // mid 101, micro 101
        ];
        // The mid changes at 4, 7 and 9.
        let mut mid = rule(2).with_price_source(MidPrice);
        assert_eq!(trigger_indices(&mut mid, &events), vec![7]);
        assert_eq!(mid.threshold(), 2.0);
        // The microprice changes at 2, 4, 5, 7, 8 and 9.
        let mut micro = rule(2).with_price_source(Microprice);
        assert_eq!(trigger_indices(&mut micro, &events), vec![4, 7, 9]);
        // The trade price changes at 3 and 6.
        assert_eq!(trigger_indices(&mut rule(2), &events), vec![6]);
    }
}
