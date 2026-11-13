use std::{cmp::Ordering, collections::BinaryHeap};

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// Wraps `f64` with the total order of [`f64::total_cmp`], so it can go into a [`BinaryHeap`].
/// Trade prices are never NaN, so this agrees with a normal numeric sort.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct TotalOrd(f64);

impl Eq for TotalOrd {}

impl PartialOrd for TotalOrd {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TotalOrd {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Computes the exact running median of the trade prices with two heaps,
/// so that `value` is O(1) and `update` is O(log n), without cloning or sorting the prices.
///
/// `lower` is a max-heap of the smaller half of the prices (plus the median for an odd count)
/// and `upper` is a max-heap of the *negated* larger half, i.e. a min-heap of the larger half.
/// Negating instead of a second [`Ord`] newtype keeps both heaps the same type, so `derive(serde)` applies.
/// `lower.len()` is always `upper.len()` or `upper.len() + 1`.
///
/// `None` for an empty candle. For an even count this returns the smallest price of the
/// larger half (the element at index `len / 2` of the sorted prices), like a plain sort would,
/// instead of averaging the two middle prices: the result is then always a price that was
/// actually traded, and the type stays `Option<f64>` instead of needing interpolation.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MedianPrice {
    lower: BinaryHeap<TotalOrd>,
    upper_negated: BinaryHeap<TotalOrd>,
}

impl MedianPrice {
    /// Adds `price` to the correct half, then restores the size invariant between the heaps.
    #[inline(always)]
    fn push(&mut self, price: f64) {
        debug_assert!(price.is_finite(), "a trade price must be finite");
        let goes_lower = self.lower.peek().is_none_or(|top| price <= top.0);
        if goes_lower {
            self.lower.push(TotalOrd(price));
        } else {
            self.upper_negated.push(TotalOrd(-price));
        }

        if self.lower.len() > self.upper_negated.len() + 1 {
            let moved = self.lower.pop().expect("lower has at least 2 elements");
            self.upper_negated.push(TotalOrd(-moved.0));
        } else if self.upper_negated.len() > self.lower.len() {
            let moved = self
                .upper_negated
                .pop()
                .expect("upper_negated has at least 1 element");
            self.lower.push(TotalOrd(-moved.0));
        }

        debug_assert!(self.lower.len() >= self.upper_negated.len());
        debug_assert!(self.lower.len() <= self.upper_negated.len() + 1);
    }
}

impl CandleComponent for MedianPrice {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if self.lower.len() > self.upper_negated.len() {
            self.lower.peek().map(|top| top.0)
        } else {
            self.upper_negated.peek().map(|top| -top.0)
        }
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.lower.clear();
        self.upper_negated.clear();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for MedianPrice {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.push(trade.price());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact median of `prices`, by sorting, with the same even-count convention as `MedianPrice`.
    fn sorted_median(prices: &[f64]) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        let mut sorted = prices.to_vec();
        sorted.sort_by(f64::total_cmp);
        Some(sorted[sorted.len() / 2])
    }

    /// Prices from a seeded xorshift generator, so the sequence is reproducible.
    fn pseudo_random_prices(n: usize, seed: u64) -> Vec<f64> {
        let mut state = seed;
        (0..n)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 11) as f64 / (1_u64 << 53) as f64 * 100.0
            })
            .collect()
    }

    #[test]
    fn median_price_matches_the_textbook_example() {
        let mut m = MedianPrice::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), Some(102.0));
    }

    #[test]
    fn an_empty_candle_has_no_median() {
        let m = MedianPrice::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn a_single_price_is_its_own_median() {
        let mut m = MedianPrice::default();
        m.push(42.0);
        assert_eq!(m.value(), Some(42.0));
    }

    #[test]
    fn an_odd_count_returns_the_middle_price() {
        let mut m = MedianPrice::default();
        for p in [5.0, 1.0, 3.0] {
            m.push(p);
        }
        assert_eq!(m.value(), Some(3.0));
    }

    #[test]
    fn an_even_count_returns_the_smallest_of_the_upper_half() {
        let mut m = MedianPrice::default();
        for p in [1.0, 2.0, 3.0, 4.0] {
            m.push(p);
        }
        // sorted [1, 2, 3, 4], index len / 2 == 2, i.e. 3.0, not the average 2.5.
        assert_eq!(m.value(), Some(3.0));
    }

    #[test]
    fn duplicate_prices_are_all_counted() {
        let mut m = MedianPrice::default();
        for p in [2.0, 2.0, 2.0, 2.0, 5.0] {
            m.push(p);
        }
        assert_eq!(m.value(), Some(2.0));
    }

    #[test]
    fn reset_forgets_every_price() {
        let mut m = MedianPrice::default();
        for p in [1.0, 2.0, 3.0] {
            m.push(p);
        }
        m.reset();
        assert_eq!(m.value(), None);
        m.push(9.0);
        assert_eq!(m.value(), Some(9.0));
    }

    #[test]
    fn matches_a_sorted_reference_on_a_pseudo_random_sequence() {
        let prices = pseudo_random_prices(2_000, 0xd1ce_5eed);
        let mut m = MedianPrice::default();
        let mut seen = Vec::new();
        for (i, &p) in prices.iter().enumerate() {
            m.push(p);
            seen.push(p);
            if i % 37 == 0 {
                assert_eq!(m.value(), sorted_median(&seen), "after {} prices", i + 1);
            }
        }
        assert_eq!(m.value(), sorted_median(&seen));
    }
}
