//! An approximate, constant-memory median, for the case of many aggregators x many
//! parameters x many symbols, where [`MedianPrice`](super::MedianPrice)'s exact heaps
//! would cost memory per candle that adds up across all of them.

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// The core of the P² algorithm (Jain and Chlamtac, 1985, CACM): an online estimator of a
/// quantile `p` in O(1) memory and O(1) time per observation, without storing the observations.
///
/// Five markers track the minimum, the `p/2`-, `p`-, `(1+p)/2`-quantile and the maximum.
/// Each new observation shifts the markers' positions, then a piecewise-parabolic formula
/// (falling back to linear) nudges a marker's height toward its ideal position when it has
/// drifted off by more than one observation.
///
/// Private, so that only [`ApproxMedianPrice`] (`p = 0.5`) is public today; a later
/// `ApproxQuantile<p>` for another quantile could reuse this core unchanged.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct P2Quantile {
    /// The quantile to estimate, in `(0, 1)`.
    p: f64,
    /// The first observations, buffered until there are five to sort and start the markers.
    init: [f64; 5],
    /// How many of `init` are valid; `5` once `markers` takes over.
    init_len: u8,
    /// `Some` once five observations started the markers.
    markers: Option<Markers>,
}

/// The five markers of the P² algorithm: heights (the quantile estimates) and their
/// positions (how many observations are at or below them), real and desired.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Markers {
    /// The marker heights, nondecreasing: minimum, `p/2`-, `p`-, `(1+p)/2`-quantile, maximum.
    height: [f64; 5],
    /// The integral marker positions, strictly increasing.
    position: [f64; 5],
    /// The real-valued desired marker positions.
    desired: [f64; 5],
    /// The constant increment of `desired` added for every observation.
    increment: [f64; 5],
}

impl P2Quantile {
    /// A fresh estimator of the `p`-quantile, `p` in `(0, 1)`.
    fn new(p: f64) -> Self {
        assert!(p > 0.0, "the quantile must be in (0, 1), got {p}");
        assert!(p < 1.0, "the quantile must be in (0, 1), got {p}");
        Self {
            p,
            init: [0.0; 5],
            init_len: 0,
            markers: None,
        }
    }

    /// Adds an observation: exact while buffering the first five, P² afterwards.
    #[inline(always)]
    fn add(&mut self, x: f64) {
        debug_assert!(x.is_finite(), "an observation must be finite");
        if let Some(markers) = &mut self.markers {
            markers.add(x);
            return;
        }
        debug_assert!(self.init_len < 5, "markers take over at 5 observations");
        self.init[self.init_len as usize] = x;
        self.init_len += 1;
        if self.init_len == 5 {
            let mut sorted = self.init;
            sorted.sort_by(f64::total_cmp);
            self.markers = Some(Markers::new(self.p, sorted));
        }
    }

    /// The current estimate, `None` without an observation yet.
    /// Exact for up to 5 observations, approximate afterwards.
    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if let Some(markers) = &self.markers {
            return Some(markers.height[2]);
        }
        let len = usize::from(self.init_len);
        if len == 0 {
            return None;
        }
        let mut sorted = self.init;
        sorted[..len].sort_by(f64::total_cmp);
        // Agrees with a plain sort for p = 0.5: index len / 2 for both an odd and an even count.
        let index = (self.p * (len - 1) as f64).round() as usize;
        debug_assert!(index < len);
        Some(sorted[index])
    }

    /// Forgets every observation.
    #[inline(always)]
    fn reset(&mut self) {
        self.init_len = 0;
        self.markers = None;
    }
}

impl Markers {
    /// Starts the markers from five already-sorted observations (step A of the paper).
    fn new(p: f64, height: [f64; 5]) -> Self {
        debug_assert!(
            height.is_sorted(),
            "the initial five markers must be sorted"
        );
        Self {
            height,
            position: [1.0, 2.0, 3.0, 4.0, 5.0],
            desired: [1.0, 1.0 + 2.0 * p, 1.0 + 4.0 * p, 3.0 + 2.0 * p, 5.0],
            increment: [0.0, p / 2.0, p, (1.0 + p) / 2.0, 1.0],
        }
    }

    /// Processes one more observation (step B of the paper).
    fn add(&mut self, x: f64) {
        let k = self.cell_of(x);
        for position in &mut self.position[k + 1..] {
            *position += 1.0;
        }
        for (desired, increment) in self.desired.iter_mut().zip(self.increment) {
            *desired += increment;
        }
        for i in 1..4 {
            self.adjust(i);
        }
        debug_assert!(self.height.is_sorted(), "marker heights must stay ordered");
        debug_assert!(
            self.position.is_sorted(),
            "marker positions must stay ordered"
        );
    }

    /// The index `k` such that `height[k] <= x < height[k + 1]`,
    /// widening the extremes (`height[0]`, `height[4]`) if `x` is beyond them.
    fn cell_of(&mut self, x: f64) -> usize {
        if x < self.height[0] {
            self.height[0] = x;
            return 0;
        }
        if x >= self.height[4] {
            self.height[4] = x;
            return 3;
        }
        (0..4)
            .find(|&i| self.height[i] <= x && x < self.height[i + 1])
            .expect("height[0] <= x < height[4] was just checked")
    }

    /// Nudges marker `i` (one of the 3 middle ones) toward its desired position,
    /// by exactly one observation, if it has drifted off by more than one.
    fn adjust(&mut self, i: usize) {
        assert!(
            (1..4).contains(&i),
            "only the 3 middle markers are adjusted"
        );
        let drift = self.desired[i] - self.position[i];
        let right_has_room = self.position[i + 1] - self.position[i] > 1.0;
        let left_has_room = self.position[i - 1] - self.position[i] < -1.0;
        if !((drift >= 1.0 && right_has_room) || (drift <= -1.0 && left_has_room)) {
            return;
        }
        let d: i32 = if drift > 0.0 { 1 } else { -1 };
        let parabolic = self.parabolic(i, d);
        self.height[i] = if self.height[i - 1] < parabolic && parabolic < self.height[i + 1] {
            parabolic
        } else {
            self.linear(i, d)
        };
        self.position[i] += f64::from(d);
    }

    /// The piecewise-parabolic prediction of the height of marker `i` moved by `d` (+-1).
    fn parabolic(&self, i: usize, d: i32) -> f64 {
        let (q, n) = (self.height, self.position);
        let d = f64::from(d);
        q[i] + d / (n[i + 1] - n[i - 1])
            * ((n[i] - n[i - 1] + d) * (q[i + 1] - q[i]) / (n[i + 1] - n[i])
                + (n[i + 1] - n[i] - d) * (q[i] - q[i - 1]) / (n[i] - n[i - 1]))
    }

    /// The linear fallback for the height of marker `i` moved by `d` (+-1),
    /// used when the parabolic prediction would break the marker order.
    fn linear(&self, i: usize, d: i32) -> f64 {
        let j = if d > 0 { i + 1 } else { i - 1 };
        self.height[i]
            + f64::from(d) * (self.height[j] - self.height[i])
                / (self.position[j] - self.position[i])
    }
}

/// An approximate median price, using the P² algorithm: O(1) memory and update, instead of
/// [`MedianPrice`](super::MedianPrice)'s exact heaps, which keep every price of the candle.
/// Exact for up to 5 trades, an estimate afterwards. `None` for a candle without trades.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ApproxMedianPrice {
    quantile: P2Quantile,
}

impl Default for ApproxMedianPrice {
    fn default() -> Self {
        Self {
            quantile: P2Quantile::new(0.5),
        }
    }
}

impl CandleComponent for ApproxMedianPrice {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.quantile.value()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.quantile.reset();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for ApproxMedianPrice {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.quantile.add(trade.price());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact median of `prices`, with the same even-count convention as `MedianPrice`:
    /// the element at index `len / 2` of the sorted prices.
    fn exact_median(prices: &[f64]) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        let mut sorted = prices.to_vec();
        sorted.sort_by(f64::total_cmp);
        Some(sorted[sorted.len() / 2])
    }

    /// Prices from a seeded xorshift generator, uniform on `[0, 100)`, reproducible.
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
    fn an_empty_candle_has_no_median() {
        let m = ApproxMedianPrice::default();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn exact_for_up_to_five_prices() {
        let prices = pseudo_random_prices(5, 0x1234_5678);
        for n in 1..=5 {
            let mut m = ApproxMedianPrice::default();
            for &p in &prices[..n] {
                m.update(&crate::Trade {
                    timestamp: 0,
                    price: p,
                    size: 1.0,
                });
            }
            assert_eq!(m.value(), exact_median(&prices[..n]), "n = {n}");
        }
    }

    #[test]
    fn reset_forgets_every_price() {
        let mut m = ApproxMedianPrice::default();
        for p in [1.0, 2.0, 3.0] {
            m.update(&crate::Trade {
                timestamp: 0,
                price: p,
                size: 1.0,
            });
        }
        m.reset();
        assert_eq!(m.value(), None);
    }

    #[test]
    fn stays_close_to_the_exact_median_on_a_long_pseudo_random_sequence() {
        let prices = pseudo_random_prices(5_000, 0xabcd_ef01);
        let mut m = ApproxMedianPrice::default();
        for &p in &prices {
            m.update(&crate::Trade {
                timestamp: 0,
                price: p,
                size: 1.0,
            });
        }
        let approx = m.value().expect("5000 prices were observed");
        let exact = exact_median(&prices).expect("5000 prices were observed");
        // Uniform on [0, 100): within 1% of the range is a generous margin for P².
        assert!(
            (approx - exact).abs() < 1.0,
            "approx {approx} too far from exact {exact}"
        );
    }
}
