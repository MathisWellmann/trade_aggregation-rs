//! Components of the sequence of the trade signs: +1 for a taker buy, -1 for a taker sell.
//!
//! Trade signs have a long memory, because large orders are split into many small trades
//! that hit the same side for a long time (Lillo and Farmer 2004, "The long memory of the
//! efficient market"). How persistent the signs are measures how one-sided the flow is
//! and how much of it is a large hidden order, which the share of buys alone does not show:
//! `+ + - -` and `+ - + -` have the same share of buys.
//!
//! A trade with a size of zero has no side, so these components skip it:
//! it neither breaks nor extends a run.

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// The side of a taker trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum Side {
    /// The taker bought, at the ask: a positive size.
    Buy,
    /// The taker sold, at the bid: a negative size.
    Sell,
}

impl Side {
    /// The side of `trade`, `None` for a trade without size.
    #[inline(always)]
    fn of<T: TakerTrade>(trade: &T) -> Option<Self> {
        let size = trade.size();
        if size > 0.0 {
            Some(Self::Buy)
        } else if size < 0.0 {
            Some(Self::Sell)
        } else {
            None
        }
    }

    /// +1 for a buy and -1 for a sell.
    #[inline(always)]
    fn sign(self) -> i64 {
        match self {
            Self::Buy => 1,
            Self::Sell => -1,
        }
    }

    /// The row or column of the side in a table of transitions.
    #[inline(always)]
    fn index(self) -> usize {
        match self {
            Self::Buy => 0,
            Self::Sell => 1,
        }
    }
}

/// The number of times the side changes from one trade to the next, including the change
/// from the last trade of the previous candle to the first trade of the candle,
/// so that the sum over candles is the number of changes of the whole series (additive).
///
/// Few switches per trade mean persistent, one-sided flow, e.g. a large order being split;
/// divide by the number of trades for a scale-free switch rate, which is about 1/2
/// for independent signs (the runs test of Wald and Wolfowitz 1940).
/// Trades without size are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignSwitches {
    switches: u64,
    last: Option<Side>,
}

impl CandleComponent for SignSwitches {
    type Value = u64;

    #[inline(always)]
    fn value(&self) -> u64 {
        self.switches
    }

    /// Keeps the side of the last trade, for the first switch of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.switches = 0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for SignSwitches {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(side) = event.as_trade().and_then(Side::of) else {
            return;
        };
        if self.last.replace(side).is_some_and(|last| last != side) {
            self.switches += 1;
        }
    }
}

/// The length of the longest run of consecutive trades of the same side,
/// counting only the trades of the candle. 0 for a candle without trades.
///
/// A long run is a large order that sweeps through the trades of the candle.
/// For `n` independent signs of a fair coin, the longest run is about `log2(n)`
/// (Schilling 1990, "The longest run of heads"), so compare it with the number of trades.
/// Trades without size are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LongestRun {
    longest: u64,
    run: u64,
    last: Option<Side>,
}

impl CandleComponent for LongestRun {
    type Value = u64;

    #[inline(always)]
    fn value(&self) -> u64 {
        self.longest
    }

    /// A run that continues from the previous candle starts again at the open.
    #[inline(always)]
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for LongestRun {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(side) = event.as_trade().and_then(Side::of) else {
            return;
        };
        if self.last.replace(side) == Some(side) {
            self.run += 1;
        } else {
            self.run = 1;
        }
        self.longest = self.longest.max(self.run);
        debug_assert!(self.run >= 1);
        debug_assert!(self.longest >= self.run);
    }
}

/// The conditional entropy `H(s_t | s_(t-1))` in bits of the side of a trade given the side
/// of the trade before, from the 2x2 table of the transitions between consecutive trades
/// (Shannon 1948). `None` for a candle without a transition.
///
/// The order-1 upgrade of the order-0 [`Entropy`](super::Entropy): alternating signs
/// (`+ - + -`) have an entropy of 1 bit but a conditional entropy of 0, because each sign
/// predicts the next. A value well below 1 means predictable, persistent or alternating flow.
/// The value is in `[0, 1]`, so it is scale-free.
///
/// A transition belongs to the candle of its second trade, so the first trade of a candle
/// counts with the last trade of the previous candle, as in [`SignSwitches`].
/// Trades without size are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConditionalSignEntropy {
    /// `transitions[from][to]` counts the trades of side `to` after a trade of side `from`.
    transitions: [[u64; 2]; 2],
    last: Option<Side>,
}

impl CandleComponent for ConditionalSignEntropy {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        let total: u64 = self.transitions.iter().flatten().sum();
        if total == 0 {
            return None;
        }
        let total = total as f64;
        let mut entropy = 0.0;
        for row in &self.transitions {
            let from = (row[0] + row[1]) as f64;
            for &count in row.iter().filter(|&&count| count > 0) {
                let count = count as f64;
                entropy -= count / total * (count / from).log2();
            }
        }
        debug_assert!(entropy >= -1e-12, "entropy {entropy}");
        debug_assert!(entropy <= 1.0 + 1e-12, "entropy {entropy}");
        Some(entropy.clamp(0.0, 1.0))
    }

    /// Keeps the side of the last trade, for the first transition of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.transitions = [[0; 2]; 2];
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for ConditionalSignEntropy {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(side) = event.as_trade().and_then(Side::of) else {
            return;
        };
        if let Some(from) = self.last.replace(side) {
            self.transitions[from.index()][side.index()] += 1;
        }
    }
}

/// The lag-1 autocorrelation of the signs `s_1..s_n` (+1 or -1) of the trades of the candle:
/// `sum_(t=2..n) (s_t - m) (s_(t-1) - m) / sum_(t=1..n) (s_t - m)^2` with the mean `m` of the
/// signs. `None` with fewer than two trades or if all trades have the same side.
///
/// Positive values mean persistent flow, as of a split order (Lillo and Farmer 2004);
/// negative values mean alternating flow, as of market makers that trade both sides.
/// Only the trades of the candle count. Trades without size are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignAutocorrelation {
    n: u64,
    /// The sum of the signs.
    sum: i64,
    /// The sum of the products of consecutive signs.
    lag_products: i64,
    first: Option<Side>,
    last: Option<Side>,
}

impl CandleComponent for SignAutocorrelation {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        if self.n < 2 || self.sum.unsigned_abs() == self.n {
            return None;
        }
        let first = self.first.expect("a candle with trades has a first side");
        let last = self.last.expect("a candle with trades has a last side");
        let n = self.n as f64;
        let sum = self.sum as f64;
        let mean = sum / n;
        // The expanded sum of (s_t - m) (s_(t-1) - m) over the n - 1 consecutive pairs.
        let covariance = self.lag_products as f64
            - mean * (2.0 * sum - first.sign() as f64 - last.sign() as f64)
            + (n - 1.0) * mean * mean;
        // The squares of the signs are 1.
        let variance = n * (1.0 - mean * mean);
        debug_assert!(variance > 0.0, "both sides traded");
        let autocorrelation = covariance / variance;
        debug_assert!(autocorrelation.abs() <= 1.0 + 1e-12, "{autocorrelation}");
        Some(autocorrelation)
    }

    #[inline(always)]
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for SignAutocorrelation {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(side) = event.as_trade().and_then(Side::of) else {
            return;
        };
        if let Some(last) = self.last.replace(side) {
            self.lag_products += last.sign() * side.sign();
        }
        self.first.get_or_insert(side);
        self.n += 1;
        self.sum += side.sign();
        debug_assert!(self.sum.unsigned_abs() <= self.n);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trade, Aggregator, Candle, Gaps, GenericAggregator,
        MarketEvent, MillisecondPeriod, Quote, TimeRule, Trade,
    };

    /// Trades one millisecond apart, a buy for `'+'`, a sell for `'-'` and no size for `'0'`.
    fn signs(pattern: &str) -> Vec<Trade> {
        pattern
            .chars()
            .zip(0..)
            .map(|(c, ts)| match c {
                '+' => trade(ts, 100.0, 1.0),
                '-' => trade(ts, 100.0, -1.0),
                '0' => trade(ts, 100.0, 0.0),
                _ => panic!("unknown sign {c}"),
            })
            .collect()
    }

    fn run<C: CandleComponent + CandleComponentUpdate<Trade> + Default>(pattern: &str) -> C {
        let mut c = C::default();
        for t in &signs(pattern) {
            c.update(t);
        }
        c
    }

    #[test]
    fn sign_switches_count_side_changes_across_candles() {
        assert_eq!(run::<SignSwitches>("").value(), 0);
        assert_eq!(run::<SignSwitches>("+").value(), 0);
        assert_eq!(run::<SignSwitches>("++--+").value(), 2);
        assert_eq!(run::<SignSwitches>("+0+0-").value(), 1, "no size, no side");
        let mut m = run::<SignSwitches>("+-");
        assert_eq!(m.value(), 1);
        m.reset();
        assert_eq!(m.value(), 0);
        for t in &signs("+") {
            m.update(t);
        }
        assert_eq!(
            m.value(),
            1,
            "the switch from the previous candle's last trade"
        );
    }

    #[test]
    fn the_longest_run_counts_within_the_candle() {
        assert_eq!(run::<LongestRun>("").value(), 0);
        assert_eq!(run::<LongestRun>("+--+++-").value(), 3);
        assert_eq!(run::<LongestRun>("--0--+").value(), 4, "no size, no break");
        let mut m = run::<LongestRun>("+++");
        m.reset();
        assert_eq!(m.value(), 0);
        for t in &signs("++-") {
            m.update(t);
        }
        assert_eq!(
            m.value(),
            2,
            "the run of the previous candle does not continue"
        );
    }

    #[test]
    fn conditional_entropy_from_the_transitions() {
        assert_eq!(run::<ConditionalSignEntropy>("").value(), None);
        assert_eq!(run::<ConditionalSignEntropy>("+").value(), None);
        // ++, +-, --, -+: each side is followed by both sides equally often.
        assert_eq!(run::<ConditionalSignEntropy>("++--+").value(), Some(1.0));
        // Alternating signs are predictable from the last one.
        assert_eq!(run::<ConditionalSignEntropy>("+-+-+").value(), Some(0.0));
        assert_eq!(run::<ConditionalSignEntropy>("-----").value(), Some(0.0));
        // ++, ++, +-: a buy is followed by a buy with probability 2/3.
        let h = run::<ConditionalSignEntropy>("+0++-")
            .value()
            .expect("transitions");
        let expected = -(2.0 / 3.0 * (2.0_f64 / 3.0).log2() + 1.0 / 3.0 * (1.0_f64 / 3.0).log2());
        assert!((h - expected).abs() < 1e-12);
    }

    #[test]
    fn conditional_entropy_counts_the_transition_into_the_candle() {
        let mut m = run::<ConditionalSignEntropy>("+-");
        m.reset();
        assert_eq!(m.value(), None, "an empty candle");
        for t in &signs("+") {
            m.update(t);
        }
        assert_eq!(m.value(), Some(0.0), "- then +, the only transition");
    }

    #[test]
    fn sign_autocorrelation_of_the_candle() {
        assert_eq!(run::<SignAutocorrelation>("").value(), None);
        assert_eq!(run::<SignAutocorrelation>("+").value(), None);
        assert_eq!(
            run::<SignAutocorrelation>("+++0+").value(),
            None,
            "one side"
        );
        // Mean 1/2: deviations 1/2, 1/2, 1/2, -3/2, so (1/4 + 1/4 - 3/4) / (3/4 + 9/4).
        let acf = run::<SignAutocorrelation>("+0++-")
            .value()
            .expect("both sides");
        assert!((acf - -1.0 / 12.0).abs() < 1e-12);
        // Mean 0: three products of -1 over four squares of 1.
        assert_eq!(run::<SignAutocorrelation>("+-+-").value(), Some(-0.75));
        // Mean 0: products 1, -1, 1 over 4.
        assert_eq!(run::<SignAutocorrelation>("++--").value(), Some(0.25));
        let mut m = run::<SignAutocorrelation>("+-+-");
        m.reset();
        assert_eq!(m.value(), None);
        for t in &signs("--") {
            m.update(t);
        }
        assert_eq!(m.value(), None, "the previous candle does not count");
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct SignCandle {
        sign_switches: SignSwitches,
        longest_run: LongestRun,
        conditional_sign_entropy: ConditionalSignEntropy,
        sign_autocorrelation: SignAutocorrelation,
    }

    #[test]
    fn quotes_are_ignored_and_empty_candles_have_no_signs() {
        type Event = MarketEvent<Trade, Quote>;
        let rule = TimeRule::new(MillisecondPeriod::from_non_zero(100));
        let mut a = GenericAggregator::<SignCandle, _, Event>::new(rule, false)
            .with_gaps(Gaps::Flat { max_candles: 3 });
        let quote = |timestamp| {
            Event::Quote(Quote {
                timestamp,
                bid_price: 99.0,
                bid_size: 1.0,
                ask_price: 101.0,
                ask_size: 1.0,
            })
        };
        let events = [
            Event::Trade(trade(10, 100.0, 1.0)),
            quote(15),
            Event::Trade(trade(20, 100.0, -1.0)),
            quote(25),
            Event::Trade(trade(30, 100.0, -1.0)),
            Event::Trade(trade(350, 100.0, 1.0)),
            Event::Trade(trade(460, 100.0, 1.0)),
        ];
        let mut bars = Vec::new();
        for e in &events {
            a.update(e, &mut bars);
        }
        assert_eq!(bars.len(), 4);
        let first = bars[0].candle();
        assert_eq!(first.sign_switches(), 1);
        assert_eq!(first.longest_run(), 2);
        // + then -, - then -: each side predicts the next.
        assert_eq!(first.conditional_sign_entropy(), Some(0.0));
        // Mean -1/3: (4/3 * -2/3 + -2/3 * -2/3) / (16/9 + 4/9 + 4/9) = -4/9 / 24/9.
        let acf = first.sign_autocorrelation().expect("both sides");
        assert!((acf - -1.0 / 6.0).abs() < 1e-12);
        for flat in &bars[1..3] {
            let c = flat.candle();
            assert_eq!(c.sign_switches(), 0);
            assert_eq!(c.longest_run(), 0);
            assert_eq!(c.conditional_sign_entropy(), None);
            assert_eq!(c.sign_autocorrelation(), None);
        }
        // The buy at 350 follows the sell at 30 over the gap.
        assert_eq!(bars[3].candle().sign_switches(), 1);
        assert_eq!(bars[3].candle().conditional_sign_entropy(), Some(0.0));
    }
}
