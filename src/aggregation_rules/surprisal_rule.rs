use crate::{AggregationRule, AsTrade, Error, Result, TakerTrade, ThresholdRule};

/// EXPERIMENTAL: an information bar that is not validated by a publication.
/// A candle closes after a fixed amount of information in the sides of the trades:
/// when the sum of their surprisal `-log2 p(side | previous side)` reaches `threshold` bits.
///
/// Trade signs have a long memory (Lillo and Farmer 2004), largely because large orders are
/// split into many trades on the same side. A predictable flow, e.g. the child orders of one
/// parent order, then carries few bits per trade, while a change of the flow pattern carries
/// many. Unlike a [`TickRule`](crate::TickRule), the candles are short when the flow is
/// surprising and long when it repeats itself; unlike a
/// [`SignedVolumeRule`](crate::SignedVolumeRule), an alternating flow is not balanced
/// but predictable.
///
/// The model is an online Markov chain of order 1: the probability of a buy after a buy and
/// after a sell are exponentially weighted averages of the observed transitions, starting at
/// 1/2, and are clamped to `[p_min, 1 - p_min]` when evaluated, so that a trade costs at most
/// `-log2 p_min` bits. Each trade is evaluated with the model before it, and only then updates it
/// (no lookahead). The side of a trade is the sign of its size; trades of size 0 have no side
/// and are skipped. The first trade only sets the context and costs nothing.
/// An order 1 model captures the persistence of the signs, not their full long memory,
/// so a flow that a better model would predict can still look surprising.
///
/// A random flow costs about 1 bit per trade, and the bits grow in proportion to the number of
/// trades: with [`Adaptive`](crate::Adaptive), use [`Scaling::Linear`](crate::Scaling::Linear).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurprisalRule {
    threshold: f64,
    alpha: f64,
    p_min: f64,
    /// The estimated probability of a buy after a buy (index 0) and after a sell (index 1).
    p_buy_after: [f64; 2],
    /// Whether the previous trade with a side was a buy.
    previous_buy: Option<bool>,
    bits: f64,
}

impl SurprisalRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `threshold`: The information in bits at which a candle closes, e.g. 100 for about
    ///     100 trades of a random flow.
    /// `alpha`: The weight of the newest transition in the estimated probabilities, in `(0, 1]`.
    /// `p_min`: The smallest probability of a side, in `(0, 0.5)`, e.g. 0.01:
    ///     a trade costs at most `-log2 p_min` bits.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite,
    /// if `alpha` is not in `(0, 1]` or if `p_min` is not in `(0, 0.5)`.
    pub fn new(threshold: f64, alpha: f64, p_min: f64) -> Result<Self> {
        if !(alpha > 0.0 && alpha <= 1.0) {
            return Err(Error::InvalidParam);
        }
        if !(p_min > 0.0 && p_min < 0.5) {
            return Err(Error::InvalidParam);
        }
        let mut rule = Self {
            threshold: 1.0,
            alpha,
            p_min,
            p_buy_after: [0.5; 2],
            previous_buy: None,
            bits: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

/// The row of the transition probabilities after the side `buy`.
fn row(buy: bool) -> usize {
    if buy {
        0
    } else {
        1
    }
}

/// The surprisal in bits of the side `buy` when a buy has the probability `p_buy`.
fn surprisal(p_buy: f64, buy: bool, p_min: f64) -> f64 {
    debug_assert!((0.0..=1.0).contains(&p_buy));
    let p = if buy { p_buy } else { 1.0 - p_buy };
    let bits = -p.clamp(p_min, 1.0 - p_min).log2();
    debug_assert!(bits > 0.0, "a clamped probability is below 1");
    debug_assert!(bits <= -p_min.log2());
    bits
}

/// The probability of a buy after learning the side `buy`.
fn learn(p_buy: f64, buy: bool, alpha: f64) -> f64 {
    let observed = if buy { 1.0 } else { 0.0 };
    let next = p_buy + alpha * (observed - p_buy);
    debug_assert!((0.0..=1.0).contains(&next));
    next
}

impl<E: AsTrade> AggregationRule<E> for SurprisalRule {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(trade) = event.as_trade() else {
            return false;
        };
        let size = trade.size();
        debug_assert!(size.is_finite(), "sizes must be finite");
        if size == 0.0 {
            return false;
        }
        let buy = size > 0.0;
        let Some(previous_buy) = self.previous_buy.replace(buy) else {
            return false;
        };
        let p_buy = &mut self.p_buy_after[row(previous_buy)];
        let bits = surprisal(*p_buy, buy, self.p_min);
        *p_buy = learn(*p_buy, buy, self.alpha);
        self.bits += bits;
        let trigger = self.bits >= self.threshold;
        if trigger {
            self.bits = 0.0;
        }
        debug_assert!(self.bits < self.threshold);
        trigger
    }

    /// Clear the bits of the candle; the model and the side of the last trade
    /// follow the stream and stay.
    fn reset(&mut self) {
        self.bits = 0.0;
    }
}

impl ThresholdRule for SurprisalRule {
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
    use crate::{
        aggregation_rules::test_utils::{random_flow, trade, trigger_indices},
        MarketEvent, Quote, Trade,
    };

    fn rule(threshold: f64, alpha: f64, p_min: f64) -> SurprisalRule {
        SurprisalRule::new(threshold, alpha, p_min).expect("valid parameters")
    }

    /// Trades of size 1 with the given sides, one millisecond apart.
    fn trades_with_sides(buys: impl Iterator<Item = bool>) -> Vec<Trade> {
        buys.zip(0..)
            .map(|(buy, ts)| trade(ts, 100.0, if buy { 1.0 } else { -1.0 }))
            .collect()
    }

    fn alternating(n: usize) -> Vec<Trade> {
        trades_with_sides((0..n).map(|i| i % 2 == 0))
    }

    #[test]
    fn new_rejects_invalid_parameters() {
        assert!(SurprisalRule::new(0.0, 0.1, 0.01).is_err());
        assert!(SurprisalRule::new(f64::INFINITY, 0.1, 0.01).is_err());
        assert!(SurprisalRule::new(10.0, 0.0, 0.01).is_err());
        assert!(SurprisalRule::new(10.0, 1.5, 0.01).is_err());
        assert!(SurprisalRule::new(10.0, f64::NAN, 0.01).is_err());
        assert!(SurprisalRule::new(10.0, 0.1, 0.0).is_err());
        assert!(SurprisalRule::new(10.0, 0.1, 0.5).is_err());
        assert!(SurprisalRule::new(10.0, 0.1, f64::NAN).is_err());
        assert!(SurprisalRule::new(10.0, 1.0, 0.49).is_ok());
    }

    #[test]
    fn bits_of_a_learned_alternation() {
        let mut r = rule(100.0, 0.5, 0.01);
        assert!(trigger_indices(&mut r, &alternating(5)).is_empty());
        // Buy (context), sell after buy at 1/2 (1 bit; p(buy | buy) becomes 1/4),
        // buy after sell at 1/2 (1 bit; p(buy | sell) becomes 3/4),
        // sell after buy at 3/4 and buy after sell at 3/4 (log2(4/3) bits each).
        let expected = 2.0 + 2.0 * (4.0_f64 / 3.0).log2();
        assert!((r.bits - expected).abs() < 1e-12, "{}", r.bits);
        assert_eq!(r.p_buy_after, [0.125, 0.875]);
    }

    #[test]
    fn fires_when_the_bits_reach_the_threshold() {
        // 1, 2 bits (fires), then log2(4/3) = 0.415, 0.830, ...
        let mut r = rule(1.9, 0.5, 0.01);
        assert_eq!(trigger_indices(&mut r, &alternating(5)), vec![2]);
    }

    #[test]
    fn a_predictable_flow_becomes_cheap() {
        let constant = trades_with_sides((0..2_000).map(|_| true));
        for trades in [alternating(2_000), constant] {
            let mut r = rule(10.0, 0.05, 0.001);
            let fired = trigger_indices(&mut r, &trades);
            // A few candles while the model learns, then none.
            assert!(!fired.is_empty());
            assert!(fired.iter().all(|i| *i < 200), "{fired:?}");
            // Each trade then costs -log2(0.999) = 0.0014 bits.
            AggregationRule::<Trade>::reset(&mut r);
            assert!(trigger_indices(&mut r, &trades[..1_000]).is_empty());
            assert!(r.bits < 1_000.0 * 0.0015, "{}", r.bits);
        }
    }

    #[test]
    fn a_random_flow_costs_about_one_bit_per_trade() {
        let trades = random_flow(100_000, 0.5, 11);
        let mut r = rule(100.0, 0.01, 0.01);
        let candles = trigger_indices(&mut r, &trades).len();
        // About 100 trades per candle; a little more, because the estimated probabilities
        // are noisy and a wrong estimate costs bits.
        assert!((980..=1_010).contains(&candles), "{candles}");
    }

    #[test]
    fn trades_without_size_and_quotes_are_skipped() {
        let mut events: Vec<MarketEvent<Trade, Quote>> = Vec::new();
        for t in alternating(5) {
            events.push(MarketEvent::Quote(Quote::default()));
            events.push(MarketEvent::Trade(trade(t.timestamp, 100.0, 0.0)));
            events.push(MarketEvent::Trade(t));
        }
        let mut r = rule(1.9, 0.5, 0.01);
        let fired: Vec<usize> = (0..events.len())
            .filter(|i| r.should_trigger(&events[*i]))
            .collect();
        // The third trade with a side, as in `fires_when_the_bits_reach_the_threshold`.
        assert_eq!(fired, vec![8]);
    }

    #[test]
    fn reset_clears_the_bits_but_keeps_the_model() {
        let trades = alternating(5);
        let mut r = rule(100.0, 0.5, 0.01);
        assert!(trigger_indices(&mut r, &trades[..3]).is_empty());
        AggregationRule::<Trade>::reset(&mut r);
        assert_eq!(r.bits, 0.0);
        // The next sell follows the last buy and is evaluated with the learned model.
        assert!(trigger_indices(&mut r, &trades[3..4]).is_empty());
        assert!(
            (r.bits - (4.0_f64 / 3.0).log2()).abs() < 1e-12,
            "{}",
            r.bits
        );
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(100.0, 0.1, 0.01);
        r.set_threshold(50.0).expect("valid threshold");
        assert_eq!(r.threshold(), 50.0);
        assert!(r.set_threshold(-1.0).is_err());
        assert_eq!(r.threshold(), 50.0);
    }
}
