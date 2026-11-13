//! The two scales realized variance, which removes the variance of the microstructure noise,
//! e.g. the bid-ask bounce of trade prices, from the realized variance.

use crate::{
    price_source::LogReturns, CandleComponent, CandleComponentUpdate, PriceSource, TradePrice,
};

/// The two scales realized variance (TSRV) of Zhang, Mykland and Aït-Sahalia (2005),
/// "A tale of two time scales": a realized variance that is robust to noise in the prices,
/// for trade prices that bounce between the bid and the ask when there are no quotes.
///
/// The realized variance of all `n` log returns of the candle, `RV_all`, adds twice the noise
/// variance per return. A sparse return between prices `K` positions apart adds the same noise,
/// but covers `K` returns of the efficient price. `RV_avg`, the sum of the squared sparse returns
/// divided by `K`, is the average realized variance of the `K` offset subgrids (every `K`-th
/// price), with `n_bar = m / K` returns per subgrid for `m` sparse returns.
/// `RV_avg - (n_bar / n) * RV_all` removes the noise. The value divides it by `1 - n_bar / n`,
/// the small-sample adjustment of the paper (also in the R package highfrequency): without it the
/// estimate is biased down by that factor, about `1 - 1 / K` (20 % for `K = 5`), because the
/// subtracted `RV_all` also holds the variance. Multiply by `1 - n_bar / n` for the raw estimate.
/// A larger `K` removes more noise but leaves fewer, longer sparse returns;
/// the paper lets `K` grow like `n^(2/3)`.
///
/// The subgrids continue over candle boundaries: each price closes the sparse return from the
/// price `K` positions before it, which can be in a previous candle, like the first return of a
/// candle starts at the last price of the previous one. No return is lost and no future price is
/// used. The first `K` prices of the series close no sparse return: over a first candle
/// `m = n - K + 1`, so `n_bar = (n - K + 1) / K` as in the window of the paper, and the value is
/// 0 until the first sparse return. After them `m = n` and the value is
/// `(sum of the squared sparse returns - RV_all) / (K - 1)`, additive over candles.
///
/// The value can be negative when the noise dominates a candle with few returns. It is not
/// clamped, as that would bias sums and averages over candles; clamp it downstream if needed.
/// 0 for a candle without returns. `K >= 2` is checked at compile time:
///
/// ```compile_fail
/// use trade_aggregation::{candle_components::TwoScaleRealizedVariance, *};
/// let mut tsrv = TwoScaleRealizedVariance::<TradePrice, 1>::default();
/// tsrv.update(&Trade { timestamp: 0, price: 100.0, size: 1.0 });
/// ```
///
/// ```
/// use trade_aggregation::{candle_components::TwoScaleRealizedVariance, *};
/// let mut tsrv = TwoScaleRealizedVariance::<TradePrice, 2>::default();
/// for (timestamp, price) in [(0, 100.0), (1, 101.0), (2, 100.0), (3, 101.0)] {
///     tsrv.update(&Trade { timestamp, price, size: 1.0 });
/// }
/// // A pure bounce: the sparse returns between every second price are zero.
/// assert!(tsrv.value() < 0.0);
/// ```
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TwoScaleRealizedVariance<P = TradePrice, const K: usize = 5> {
    returns: LogReturns<P>,
    /// The last `K` prices of the series, a ring buffer: a `Vec` because serde does not
    /// implement its traits for arrays of a generic length.
    recent: Vec<f64>,
    /// The index in `recent` of the price `K` positions before the next one, once it is full.
    oldest: usize,
    /// `RV_all`, the sum of the squared log returns of the candle.
    all: f64,
    /// The number `n` of log returns of the candle.
    n: u64,
    /// The sum of the squared sparse log returns that end in the candle.
    sparse: f64,
    /// The number `m` of sparse log returns that end in the candle.
    m: u64,
}

impl<P, const K: usize> TwoScaleRealizedVariance<P, K> {
    /// The log return from the price `K` positions before `price`, `None` in the first `K`
    /// prices of the series. Stores `price` in place of that one.
    #[inline(always)]
    fn sparse_return(&mut self, price: f64) -> Option<f64> {
        debug_assert!(self.recent.len() <= K);
        if self.recent.len() < K {
            self.recent.reserve_exact(K - self.recent.len());
            self.recent.push(price);
            return None;
        }
        assert!(self.oldest < K, "the ring buffer holds K prices");
        let before = std::mem::replace(&mut self.recent[self.oldest], price);
        self.oldest = (self.oldest + 1) % K;
        Some((price / before).ln())
    }
}

impl<P, const K: usize> CandleComponent for TwoScaleRealizedVariance<P, K> {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        const { assert!(K >= 2, "the sparse subgrids need K >= 2") };
        assert!(
            self.m <= self.n,
            "a sparse return ends at a price with a return"
        );
        if self.n == 0 {
            return 0.0;
        }
        let (n, m) = (self.n as f64, self.m as f64);
        // (RV_avg - n_bar / n * RV_all) / (1 - n_bar / n) with RV_avg = sparse / K, n_bar = m / K.
        let denominator = K as f64 * n - m;
        debug_assert!(denominator >= n, "m <= n and K >= 2");
        (n * self.sparse - m * self.all) / denominator
    }

    /// Keeps the last `K` prices, for the first returns of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.all = 0.0;
        self.n = 0;
        self.sparse = 0.0;
        self.m = 0;
    }
}

impl<E, P: PriceSource<E>, const K: usize> CandleComponentUpdate<E>
    for TwoScaleRealizedVariance<P, K>
{
    #[inline(always)]
    fn update(&mut self, event: &E) {
        const { assert!(K >= 2, "the sparse subgrids need K >= 2") };
        let Some(step) = self.returns.observe(event) else {
            return;
        };
        if let Some(r) = step.log_return {
            self.all += r * r;
            self.n += 1;
        }
        if let Some(r) = self.sparse_return(step.price) {
            self.sparse += r * r;
            self.m += 1;
        }
        assert!(
            self.m <= self.n,
            "a sparse return ends at a price with a return"
        );
        debug_assert!(self.all >= 0.0);
        debug_assert!(self.sparse >= 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trades_at_prices, candle_components::RealizedVariance,
        MarketEvent, Microprice, MidPrice, Quote, Trade,
    };

    fn update_all<E, C: CandleComponentUpdate<E>>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    fn realized_variance(prices: &[f64]) -> f64 {
        prices.windows(2).map(|w| (w[1] / w[0]).ln().powi(2)).sum()
    }

    /// The adjusted TSRV of a window that starts with its first price, as in the paper:
    /// the subgrid with offset `j` holds the prices `j, j + k, j + 2k, ...`.
    fn offline_tsrv(prices: &[f64], k: usize) -> f64 {
        let all = realized_variance(prices);
        let average = (0..k)
            .map(|offset| {
                let subgrid: Vec<f64> = prices[offset..].iter().copied().step_by(k).collect();
                realized_variance(&subgrid)
            })
            .sum::<f64>()
            / k as f64;
        let n = (prices.len() - 1) as f64;
        let n_bar = (n - k as f64 + 1.0) / k as f64;
        (average - n_bar / n * all) / (1.0 - n_bar / n)
    }

    #[test]
    fn a_short_path_by_hand() {
        let prices = [100.0_f64, 101.0, 99.0, 99.0, 102.0];
        let mut tsrv = TwoScaleRealizedVariance::<TradePrice, 2>::default();
        update_all(&mut tsrv, &trades_at_prices(&prices));
        // n = 4 returns, m = 3 sparse returns: 100 -> 99, 101 -> 99, 99 -> 102.
        let all = realized_variance(&prices);
        let sparse = [99.0_f64 / 100.0, 99.0 / 101.0, 102.0 / 99.0]
            .map(|g| g.ln().powi(2))
            .iter()
            .sum::<f64>();
        let expected = (4.0 * sparse - 3.0 * all) / (2.0 * 4.0 - 3.0);
        assert!((tsrv.value() - expected).abs() < 1e-15);
        assert!((tsrv.value() - offline_tsrv(&prices, 2)).abs() < 1e-15);
    }

    #[test]
    fn no_returns_no_variance() {
        let mut tsrv = TwoScaleRealizedVariance::<TradePrice>::default();
        assert_eq!(tsrv.value(), 0.0);
        update_all(&mut tsrv, &trades_at_prices(&[100.0]));
        assert_eq!(tsrv.value(), 0.0, "the first price has no return");
        update_all(&mut tsrv, &trades_at_prices(&[101.0, 100.0, 102.0, 101.0]));
        assert_eq!(tsrv.value(), 0.0, "no sparse return in the first K prices");
        update_all(&mut tsrv, &trades_at_prices(&[103.0]));
        assert!(tsrv.value() > 0.0, "the first sparse return, 100 -> 103");
        tsrv.reset();
        assert_eq!(tsrv.value(), 0.0, "an empty candle");
        // The next candle continues the subgrids: 101 -> 104 is K = 5 prices apart.
        update_all(&mut tsrv, &trades_at_prices(&[104.0]));
        let (r, sparse) = ((104.0_f64 / 103.0).ln(), (104.0_f64 / 101.0).ln());
        let expected = (sparse * sparse - r * r) / 4.0;
        assert!((tsrv.value() - expected).abs() < 1e-15);

        let quote = MarketEvent::<Trade, Quote>::Quote(Quote::default());
        let mut trades = TwoScaleRealizedVariance::<TradePrice, 3>::default();
        trades.update(&quote);
        assert_eq!(trades.value(), 0.0, "a quote has no trade price");
    }

    /// A seeded xorshift generator of uniform numbers in `(0, 1]`.
    struct Uniform(u64);

    impl Uniform {
        fn sample(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            ((self.0 >> 11) + 1) as f64 / (1_u64 << 53) as f64
        }

        /// A standard normal deviate (Box-Muller).
        fn normal(&mut self) -> f64 {
            let (u, v) = (self.sample(), self.sample());
            (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
        }
    }

    /// `n + 1` efficient prices of a random walk of the log price with steps of `sigma`.
    fn random_walk(n: usize, sigma: f64, seed: u64) -> Vec<f64> {
        let mut uniform = Uniform(seed);
        let mut log_price = 100.0_f64.ln();
        let mut prices = vec![100.0];
        for _ in 0..n {
            log_price += sigma * uniform.normal();
            prices.push(log_price.exp());
        }
        prices
    }

    /// Trade prices that hit the bid or the ask at random, `half_spread` (relative) away.
    fn bounce(prices: &[f64], half_spread: f64, seed: u64) -> Vec<f64> {
        let mut uniform = Uniform(seed);
        prices
            .iter()
            .map(|p| {
                let side = if uniform.sample() < 0.5 { -1.0 } else { 1.0 };
                p * (1.0 + side * half_spread)
            })
            .collect()
    }

    #[test]
    fn the_first_candle_matches_the_paper() {
        let prices = bounce(&random_walk(500, 1e-4, 3), 2e-4, 5);
        let mut tsrv = TwoScaleRealizedVariance::<TradePrice>::default();
        update_all(&mut tsrv, &trades_at_prices(&prices));
        let expected = offline_tsrv(&prices, 5);
        assert!(
            (tsrv.value() - expected).abs() < 1e-12 * expected.abs(),
            "{expected}"
        );
    }

    #[test]
    fn subgrids_continue_over_candle_boundaries() {
        let trades = trades_at_prices(&random_walk(300, 1e-3, 11));
        // After the first K prices every price closes a return and a sparse return,
        // so the value is additive over candles.
        let warm_up = |tsrv: &mut TwoScaleRealizedVariance<TradePrice, 4>| {
            update_all(tsrv, &trades[..10]);
            tsrv.reset();
        };
        let mut whole = TwoScaleRealizedVariance::<TradePrice, 4>::default();
        warm_up(&mut whole);
        update_all(&mut whole, &trades[10..]);
        for at in [11, 12, 13, 50, 299] {
            let mut split = TwoScaleRealizedVariance::<TradePrice, 4>::default();
            warm_up(&mut split);
            update_all(&mut split, &trades[10..at]);
            let first = split.value();
            split.reset();
            update_all(&mut split, &trades[at..]);
            let sum = first + split.value();
            assert!((sum - whole.value()).abs() < 1e-14, "{sum} at {at}");
        }
    }

    #[test]
    fn two_scales_remove_the_bid_ask_bounce() {
        let efficient = random_walk(20_000, 1e-4, 17);
        let variance = realized_variance(&efficient);
        let trades = trades_at_prices(&bounce(&efficient, 2e-4, 19));
        let mut rv = RealizedVariance::<TradePrice>::default();
        let mut tsrv = TwoScaleRealizedVariance::<TradePrice>::default();
        update_all(&mut rv, &trades);
        update_all(&mut tsrv, &trades);
        // The bounce adds 2 * (2e-4)^2 per return, 8 times the variance of 1e-8 per return,
        // so the realized variance is about 9 times too large; the TSRV is within a few percent.
        assert!(rv.value() > 6.0 * variance, "{} vs {variance}", rv.value());
        let error = (tsrv.value() - variance).abs();
        assert!(error < 0.1 * variance, "{} vs {variance}", tsrv.value());
    }

    type Event = MarketEvent<Trade, Quote>;

    /// Quotes around the efficient price with random queue sizes, each followed by a trade
    /// that hits the bid or the ask at random.
    fn quotes_and_trades(efficient: &[f64], seed: u64) -> Vec<Event> {
        let mut uniform = Uniform(seed);
        let mut events = Vec::new();
        for (ts, p) in (0..).zip(efficient) {
            let (bid, ask) = (p * (1.0 - 2e-4), p * (1.0 + 2e-4));
            events.push(Event::Quote(Quote {
                timestamp: ts,
                bid_price: bid,
                bid_size: uniform.sample(),
                ask_price: ask,
                ask_size: uniform.sample(),
            }));
            let price = if uniform.sample() < 0.5 { bid } else { ask };
            events.push(Event::Trade(Trade {
                timestamp: ts,
                price,
                size: 1.0,
            }));
        }
        events
    }

    #[test]
    fn each_price_source_has_its_own_noise() {
        let efficient = random_walk(20_000, 1e-4, 23);
        let variance = realized_variance(&efficient);
        let events = quotes_and_trades(&efficient, 29);
        fn run<C: CandleComponent<Value = f64> + CandleComponentUpdate<Event> + Default>(
            events: &[Event],
        ) -> f64 {
            let mut c = C::default();
            update_all(&mut c, events);
            c.value()
        }
        // The mid price has no noise: both estimate the variance.
        let mid = run::<TwoScaleRealizedVariance<MidPrice>>(&events);
        assert!(
            (mid - variance).abs() < 0.1 * variance,
            "{mid} vs {variance}"
        );
        assert!((run::<RealizedVariance<MidPrice>>(&events) - variance).abs() < 1e-12);
        // The trades bounce and the microprice moves with the queues.
        for (rv, tsrv) in [
            (
                run::<RealizedVariance<TradePrice>>(&events),
                run::<TwoScaleRealizedVariance<TradePrice>>(&events),
            ),
            (
                run::<RealizedVariance<Microprice>>(&events),
                run::<TwoScaleRealizedVariance<Microprice>>(&events),
            ),
        ] {
            assert!(rv > 2.0 * variance, "{rv} vs {variance}");
            assert!(
                (tsrv - variance).abs() < 0.1 * variance,
                "{tsrv} vs {variance}"
            );
        }
    }
}
