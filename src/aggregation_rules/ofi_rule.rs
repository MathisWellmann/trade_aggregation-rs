use crate::{
    aggregation_rules::signal::OfiStep, AggregationRule, AsQuote, Error, QuoteOfi, Result,
    ThresholdRule,
};

/// How an [`OfiRule`] scales the order flow imbalance increments before it sums them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum OfiNormalization {
    /// Sum the increments in the unit of the quote sizes, e.g. contracts or coins.
    /// The threshold then depends on the typical size of the quotes of the symbol.
    Raw,
    /// Divide each increment by the depth `(bid_size + ask_size) / 2` of the quote before it.
    ///
    /// Cont, Kukanov and Stoikov (2014) find the change of the mid price linear in the order
    /// flow imbalance, with a slope inversely proportional to the depth of the book.
    /// The normalised sum therefore approximates the price impact of the flow,
    /// and one threshold is comparable across symbols and across quiet and busy hours.
    /// The paper divides by the average depth over the interval; this rule divides each
    /// increment by the depth before it, so that it needs no later quote (no lookahead).
    /// An update after a quote without any size has no depth to divide by and is skipped.
    Depth,
}

/// Creates order flow imbalance ("quote delta") bars: a candle closes when the sum of the
/// order flow imbalance increments of its quote updates (Cont, Kukanov and Stoikov 2014,
/// see [`QuoteOfi`]) reaches `threshold` in either direction.
///
/// It is the counterpart on the quotes of the [`SignedVolumeRule`](crate::SignedVolumeRule)
/// on the trades. The order flow imbalance also counts the limit orders and cancellations,
/// most of the order flow of a liquid market, and it explains the changes of the mid price
/// better than the trade imbalance does (Cont, Kukanov and Stoikov 2014).
/// Balanced quoting keeps a candle open, a one-sided flow closes it quickly.
///
/// The rule acts on the quotes and ignores other events, e.g. the trades of a
/// [`MarketEvent`](crate::MarketEvent). It skips quotes with an invalid bid or ask, or with
/// a negative or non-finite size; the first valid quote only sets the reference.
/// The sum is a random walk: with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Diffusive`](crate::Scaling::Diffusive), as for the `SignedVolumeRule`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OfiRule {
    threshold: f64,
    normalization: OfiNormalization,
    ofi: QuoteOfi,
    sum: f64,
}

impl OfiRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold`: The absolute sum of the increments at which a candle closes,
    ///     in the unit of the quote sizes or in units of depth, see `normalization`.
    /// `normalization`: How each increment is scaled before it is summed.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64, normalization: OfiNormalization) -> Result<Self> {
        let mut rule = Self {
            threshold: 1.0,
            normalization,
            ofi: QuoteOfi::new(),
            sum: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

/// The contribution of one quote update to the sum, `None` if it has none.
fn contribution(step: OfiStep, normalization: OfiNormalization) -> Option<f64> {
    debug_assert!(step.increment.is_finite());
    debug_assert!(step.depth >= 0.0);
    let value = match normalization {
        OfiNormalization::Raw => step.increment,
        OfiNormalization::Depth => step.increment / step.depth,
    };
    // A zero (or subnormal) depth leaves nothing to divide by.
    value.is_finite().then_some(value)
}

impl<E: AsQuote> AggregationRule<E> for OfiRule {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(value) = self
            .ofi
            .step(event)
            .and_then(|step| contribution(step, self.normalization))
        else {
            return false;
        };
        self.sum += value;
        let trigger = self.sum.abs() >= self.threshold;
        if trigger {
            self.sum = 0.0;
        }
        debug_assert!(self.sum.abs() < self.threshold);
        trigger
    }

    /// Clear the sum; the last quote stays the reference for the next increment.
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl ThresholdRule for OfiRule {
    fn threshold(&self) -> f64 {
        self.threshold
    }

    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        self.threshold = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{aggregation_rules::test_utils::trade, MarketEvent, Quote, Trade};

    type Event = MarketEvent<Trade, Quote>;

    fn quote(bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> Event {
        Event::Quote(Quote {
            timestamp: 0,
            bid_price: bid,
            bid_size,
            ask_price: ask,
            ask_size,
        })
    }

    fn rule(threshold: f64, normalization: OfiNormalization) -> OfiRule {
        OfiRule::new(threshold, normalization).expect("valid threshold")
    }

    fn fired(rule: &mut OfiRule, events: &[Event]) -> Vec<usize> {
        (0..events.len())
            .filter(|i| rule.should_trigger(&events[*i]))
            .collect()
    }

    /// Quotes whose increments are +5, +1, -4 and -3 with sizes scaled by `scale`,
    /// at depths 2.5, 3, 1 and 2.5 before each update.
    fn quotes(scale: f64) -> Vec<Event> {
        [
            (100.0, 2.0, 101.0, 3.0),
            (100.0, 5.0, 101.0, 1.0),
            (100.5, 1.0, 101.0, 1.0),
            (100.5, 1.0, 100.8, 4.0),
            (100.5, 1.0, 100.6, 3.0),
        ]
        .iter()
        .map(|(bid, bid_size, ask, ask_size)| quote(*bid, scale * bid_size, *ask, scale * ask_size))
        .collect()
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        for normalization in [OfiNormalization::Raw, OfiNormalization::Depth] {
            assert!(OfiRule::new(0.0, normalization).is_err());
            assert!(OfiRule::new(-1.0, normalization).is_err());
            assert!(OfiRule::new(f64::NAN, normalization).is_err());
            assert!(OfiRule::new(f64::INFINITY, normalization).is_err());
        }
    }

    #[test]
    fn raw_sums_fire_in_both_directions() {
        // Sum: 5, 6 (fires), -4, -7 (fires).
        let mut r = rule(6.0, OfiNormalization::Raw);
        assert_eq!(fired(&mut r, &quotes(1.0)), vec![2, 4]);
    }

    #[test]
    fn depth_normalised_sums_do_not_depend_on_the_size_unit() {
        // Sum: 5 / 2.5 = 2 (fires), then 1 / 3, 1 / 3 - 4 / 1 (fires), -3 / 2.5.
        for scale in [1.0, 1_000.0] {
            let mut r = rule(2.0, OfiNormalization::Depth);
            assert_eq!(fired(&mut r, &quotes(scale)), vec![1, 3]);
        }
        // The raw sums of the larger sizes fire on every update.
        let mut raw = rule(2.0, OfiNormalization::Raw);
        assert_eq!(fired(&mut raw, &quotes(1_000.0)), vec![1, 2, 3, 4]);
    }

    #[test]
    fn an_update_after_an_empty_book_is_skipped_when_normalised() {
        let events = [
            quote(100.0, 0.0, 101.0, 0.0),
            quote(100.0, 5.0, 101.0, 0.0),
            quote(100.0, 6.0, 101.0, 0.0),
        ];
        // The second update counts: +1 at a depth of 2.5.
        let mut r = rule(0.4, OfiNormalization::Depth);
        assert_eq!(fired(&mut r, &events), vec![2]);
        let mut raw = rule(0.4, OfiNormalization::Raw);
        assert_eq!(fired(&mut raw, &events), vec![1, 2]);
    }

    #[test]
    fn trades_and_invalid_quotes_are_ignored() {
        let q = quotes(1.0);
        let events = [
            Event::Trade(trade(0, 100.0, 50.0)),
            q[0],
            Event::Trade(trade(0, 100.0, -50.0)),
            quote(0.0, 1.0, 101.0, 1.0),
            quote(100.0, -1.0, 101.0, 1.0),
            q[1],
            q[2],
        ];
        let mut r = rule(6.0, OfiNormalization::Raw);
        assert_eq!(fired(&mut r, &events), vec![6]);
    }

    #[test]
    fn plain_quotes_work_without_a_market_event() {
        let quotes = [
            Quote {
                timestamp: 0,
                bid_price: 100.0,
                bid_size: 1.0,
                ask_price: 101.0,
                ask_size: 1.0,
            },
            Quote {
                timestamp: 1,
                bid_price: 100.0,
                bid_size: 3.0,
                ask_price: 101.0,
                ask_size: 1.0,
            },
        ];
        let mut r = rule(2.0, OfiNormalization::Depth);
        assert!(!r.should_trigger(&quotes[0]));
        assert!(r.should_trigger(&quotes[1]));
    }

    #[test]
    fn reset_clears_the_sum_but_keeps_the_last_quote() {
        let q = quotes(1.0);
        let mut r = rule(6.0, OfiNormalization::Raw);
        assert!(fired(&mut r, &q[..2]).is_empty());
        AggregationRule::<Event>::reset(&mut r);
        // +1 from the last quote, not +6 from the start.
        assert!(fired(&mut r, &q[2..3]).is_empty());
        assert_eq!(r.sum, 1.0);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(6.0, OfiNormalization::Depth);
        r.set_threshold(2.0).expect("valid threshold");
        assert_eq!(r.threshold(), 2.0);
        assert!(r.set_threshold(0.0).is_err());
        assert_eq!(r.threshold(), 2.0);
    }
}
