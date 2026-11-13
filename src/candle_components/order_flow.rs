//! Components of the signed order flow of the taker trades: the price impact of the flow
//! (Kyle's lambda).
//!
//! A trade with a positive size is a taker buy, one with a negative size a taker sell.
//! A trade with a size of zero has no side and moves no flow, so these components ignore it.
//!
//! Easley, López de Prado, O'Hara and Zhang (2021, "Microstructure in the machine age",
//! Review of Financial Studies) found Kyle's lambda, Amihud's lambda, the Roll measure and
//! VPIN to be useful features to predict the volatility, the spread and the sign of returns.
//! Two of them follow from other components:
//! - Amihud's lambda is the sum of the absolute log returns divided by the
//!   [`QuoteVolume`](super::QuoteVolume) of the candle: the price move per unit of notional.
//! - VPIN is the rolling mean of `|buys - sells| / volume` over the last candles of a volume rule,
//!   e.g. `|NetFlow| / BaseVolume` or `|VolumeBuys - VolumeSells| / Volume`.

use crate::{
    price_source::LogReturns, AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade,
    TradePrice,
};

/// Below this fraction of `sum x^2` the centered `sum (x - mean)^2` counts as zero:
/// the raw sums of equal values cancel up to rounding errors, which would give a huge slope.
const RELATIVE_TOLERANCE: f64 = 1e-9;

/// The additive sufficient statistics of an ordinary least squares regression of `y` on `x`
/// with an intercept: the number of observations and the sums of `x`, `y`, `x^2`, `x * y`
/// and `y^2`.
///
/// They are additive: the sums of several candles ([`Self::merge`]) are the sums of all their
/// observations, so the slope over any window of candles is exact, not an average of slopes.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegressionSums {
    /// The number of observations.
    pub n: u64,
    /// The sum of the regressors `x`.
    pub sum_x: f64,
    /// The sum of the responses `y`.
    pub sum_y: f64,
    /// The sum of `x^2`.
    pub sum_xx: f64,
    /// The sum of `x * y`.
    pub sum_xy: f64,
    /// The sum of `y^2`.
    pub sum_yy: f64,
}

impl RegressionSums {
    /// Add the observations of `other`, e.g. of the next candle.
    #[inline(always)]
    pub fn merge(&mut self, other: &Self) {
        self.n += other.n;
        self.sum_x += other.sum_x;
        self.sum_y += other.sum_y;
        self.sum_xx += other.sum_xx;
        self.sum_xy += other.sum_xy;
        self.sum_yy += other.sum_yy;
    }

    /// The least squares slope `cov(x, y) / var(x)`,
    /// `None` with fewer than two observations or if all `x` are equal.
    #[must_use]
    pub fn slope(&self) -> Option<f64> {
        let (sxx, sxy, _) = self.centered()?;
        if sxx <= RELATIVE_TOLERANCE * self.sum_xx {
            return None;
        }
        let slope = sxy / sxx;
        debug_assert!(slope.is_finite());
        Some(slope)
    }

    /// The coefficient of determination `cov(x, y)^2 / (var(x) * var(y))` in `[0, 1]`:
    /// the share of the variance of `y` that `x` explains.
    /// `None` with fewer than two observations or if all `x` or all `y` are equal.
    #[must_use]
    pub fn r_squared(&self) -> Option<f64> {
        let (sxx, sxy, syy) = self.centered()?;
        if sxx <= RELATIVE_TOLERANCE * self.sum_xx || syy <= RELATIVE_TOLERANCE * self.sum_yy {
            return None;
        }
        let r_squared = (sxy * sxy / (sxx * syy)).clamp(0.0, 1.0);
        debug_assert!(r_squared.is_finite());
        Some(r_squared)
    }

    /// The centered sums `sum (x - mean x)^2`, `sum (x - mean x) (y - mean y)` and
    /// `sum (y - mean y)^2`, `None` with fewer than two observations.
    fn centered(&self) -> Option<(f64, f64, f64)> {
        if self.n < 2 {
            return None;
        }
        let n = self.n as f64;
        let sxx = self.sum_xx - self.sum_x * self.sum_x / n;
        let sxy = self.sum_xy - self.sum_x * self.sum_y / n;
        let syy = self.sum_yy - self.sum_y * self.sum_y / n;
        Some((sxx, sxy, syy))
    }

    #[inline(always)]
    fn add(&mut self, x: f64, y: f64) {
        debug_assert!(x.is_finite(), "x = {x}");
        debug_assert!(y.is_finite(), "y = {y}");
        self.n += 1;
        self.sum_x += x;
        self.sum_y += y;
        self.sum_xx += x * x;
        self.sum_xy += x * y;
        self.sum_yy += y * y;
        debug_assert!(self.sum_xx >= 0.0);
        debug_assert!(self.sum_yy >= 0.0);
    }
}

/// The sums of the regression of Kyle's lambda ([`KyleLambda`]) of the candle:
/// `x` is the signed notional ([`TakerTrade::quote_size`]) of a trade and `y` the log return
/// from the price of the previous trade to the price of the trade.
///
/// Additive: [`RegressionSums::merge`] the values of consecutive candles to get Kyle's lambda
/// ([`RegressionSums::slope`]) and how much of the returns the flow explains
/// ([`RegressionSums::r_squared`]) over any window of candles, exactly.
///
/// The first return of a candle starts at the last trade of the previous candle,
/// so no observation is lost at a boundary. Trades without size are ignored, also as the start
/// of the next return.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KyleLambdaStats {
    sums: RegressionSums,
    returns: LogReturns<TradePrice>,
}

impl CandleComponent for KyleLambdaStats {
    type Value = RegressionSums;

    #[inline(always)]
    fn value(&self) -> RegressionSums {
        self.sums
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sums = RegressionSums::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for KyleLambdaStats {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let flow = trade.quote_size();
        if flow == 0.0 {
            return;
        }
        if let Some(r) = self.returns.observe(trade).and_then(|step| step.log_return) {
            self.sums.add(flow, r);
        }
    }
}

/// Kyle's lambda: the price impact of the order flow, as the least squares slope of the log
/// return from one trade to the next on the signed notional of the trade
/// (Kyle 1985, "Continuous auctions and insider trading").
///
/// A high lambda means a thin market, in which a given flow moves the price far,
/// and it rises when the market makers fear informed traders.
/// It is in log return per unit of notional; divide by the volatility or multiply by the
/// typical notional for a scale-free value.
/// `None` with fewer than two returns or if all trades have the same signed notional.
///
/// The first return of a candle starts at the last trade of the previous candle.
/// Trades without size are ignored. For the lambda over a window of candles,
/// sum the [`KyleLambdaStats`] instead of averaging this value.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KyleLambda {
    stats: KyleLambdaStats,
}

impl CandleComponent for KyleLambda {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.stats.value().slope()
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.stats.reset();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for KyleLambda {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        self.stats.update(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trade, Aggregator, Candle, Gaps, GenericAggregator,
        MarketEvent, MillisecondPeriod, Quote, SizeUnit, TimeRule, TimestampResolution,
        Timestamped, Trade,
    };

    /// A spot trade: the size is in the base currency.
    #[derive(Debug, Clone, Copy)]
    struct Spot {
        price: f64,
        size: f64,
    }

    impl Timestamped for Spot {
        const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Millisecond;

        fn timestamp(&self) -> i64 {
            0
        }
    }

    impl TakerTrade for Spot {
        const SIZE_UNIT: SizeUnit = SizeUnit::Base;

        fn price(&self) -> f64 {
            self.price
        }

        fn size(&self) -> f64 {
            self.size
        }
    }

    fn update_all<C: CandleComponentUpdate<E>, E>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    /// The least squares slope by the two-pass formula.
    fn ols_slope(x: &[f64], y: &[f64]) -> f64 {
        let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
        let (mx, my) = (mean(x), mean(y));
        let sxy: f64 = x.iter().zip(y).map(|(x, y)| (x - mx) * (y - my)).sum();
        let sxx: f64 = x.iter().map(|x| (x - mx) * (x - mx)).sum();
        sxy / sxx
    }

    fn sums(points: &[(f64, f64)]) -> RegressionSums {
        let mut s = RegressionSums::default();
        for &(x, y) in points {
            s.add(x, y);
        }
        s
    }

    #[test]
    fn regression_sums_give_the_slope_and_r_squared() {
        // Centered: sxx = 14 - 36 / 3 = 2, sxy = 31 - 6 * 13 / 3 = 5, syy = 69 - 169 / 3 = 38 / 3.
        let s = sums(&[(1.0, 2.0), (2.0, 4.0), (3.0, 7.0)]);
        assert_eq!(s.n, 3);
        assert_eq!(s.sum_xy, 31.0);
        assert!((s.slope().expect("x varies") - 2.5).abs() < 1e-12);
        let r2 = s.r_squared().expect("x and y vary");
        assert!((r2 - 75.0 / 76.0).abs() < 1e-12);

        let line = sums(&[(1.0, 3.0), (-2.0, -3.0), (4.0, 9.0)]);
        assert!((line.slope().expect("x varies") - 2.0).abs() < 1e-12);
        assert!((line.r_squared().expect("x and y vary") - 1.0).abs() < 1e-12);
    }

    #[test]
    fn regression_sums_merge_to_the_sums_of_all_observations() {
        let points = [(1.0, 2.0), (-2.0, 4.0), (3.0, -7.0), (0.5, 0.1), (4.0, 1.0)];
        let whole = sums(&points);
        let mut merged = sums(&points[..2]);
        merged.merge(&sums(&points[2..]));
        assert_eq!(merged.n, whole.n);
        let (a, b) = (merged.slope(), whole.slope());
        assert!((a.expect("x varies") - b.expect("x varies")).abs() < 1e-12);
        let mut empty = RegressionSums::default();
        empty.merge(&whole);
        assert_eq!(empty, whole);
    }

    #[test]
    fn degenerate_regressions_have_no_slope() {
        assert_eq!(RegressionSums::default().slope(), None);
        assert_eq!(RegressionSums::default().r_squared(), None);
        assert_eq!(sums(&[(1.0, 2.0)]).slope(), None, "one observation");
        let same_x = sums(&[(0.1, 2.0), (0.1, 4.0), (0.1, 1.0)]);
        assert_eq!(same_x.slope(), None, "x does not vary");
        assert_eq!(same_x.r_squared(), None);
        let same_y = sums(&[(1.0, 0.3), (2.0, 0.3), (5.0, 0.3)]);
        assert!(same_y.slope().expect("x varies").abs() < 1e-12);
        assert_eq!(same_y.r_squared(), None, "y does not vary");
    }

    const PRICES: [f64; 5] = [100.0, 101.0, 100.0, 102.0, 99.0];
    const NOTIONALS: [f64; 5] = [5.0, 10.0, -10.0, 20.0, -30.0];

    fn inverse_trades() -> Vec<Trade> {
        PRICES
            .iter()
            .zip(NOTIONALS)
            .zip(0..)
            .map(|((&p, q), ts)| trade(ts, p, q))
            .collect()
    }

    fn expected_lambda() -> f64 {
        let r: Vec<f64> = PRICES.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        ols_slope(&NOTIONALS[1..], &r)
    }

    #[test]
    fn kyle_lambda_regresses_returns_on_signed_notional_in_both_units() {
        let mut inverse = KyleLambda::default();
        update_all(&mut inverse, &inverse_trades());
        let lambda = inverse.value().expect("four returns");
        assert!((lambda - expected_lambda()).abs() < 1e-15);
        assert!(lambda > 0.0, "buys lift the price");

        let spot: Vec<Spot> = PRICES
            .iter()
            .zip(NOTIONALS)
            .map(|(&price, q)| Spot {
                price,
                size: q / price,
            })
            .collect();
        let mut linear = KyleLambda::default();
        update_all(&mut linear, &spot);
        let linear = linear.value().expect("four returns");
        assert!((linear - expected_lambda()).abs() < 1e-12);
    }

    #[test]
    fn kyle_lambda_starts_at_the_previous_trade_and_merges_over_candles() {
        let trades = inverse_trades();
        let mut stats = KyleLambdaStats::default();
        let mut lambda = KyleLambda::default();
        update_all(&mut stats, &trades[..2]);
        update_all(&mut lambda, &trades[..2]);
        let first = stats.value();
        assert_eq!(first.n, 1, "the first trade has no return");
        assert_eq!(lambda.value(), None, "one return");
        stats.reset();
        lambda.reset();
        assert_eq!(stats.value(), RegressionSums::default());
        assert_eq!(lambda.value(), None, "an empty candle");
        update_all(&mut stats, &trades[2..]);
        update_all(&mut lambda, &trades[2..]);
        let second = stats.value();
        assert_eq!(
            second.n, 3,
            "the boundary return belongs to the second candle"
        );
        assert!(lambda.value().is_some());
        let mut window = first;
        window.merge(&second);
        let merged = window.slope().expect("four returns");
        assert!((merged - expected_lambda()).abs() < 1e-15);
    }

    #[test]
    fn kyle_lambda_ignores_quotes_and_trades_without_size() {
        let quote = MarketEvent::Quote(Quote {
            timestamp: 0,
            bid_price: 50.0,
            bid_size: 1.0,
            ask_price: 150.0,
            ask_size: 1.0,
        });
        let mut events = vec![quote];
        for t in inverse_trades() {
            events.push(MarketEvent::Trade(t));
            events.push(MarketEvent::Trade(trade(t.timestamp, 70.0, 0.0)));
            events.push(quote);
        }
        let mut m = KyleLambda::default();
        update_all::<_, MarketEvent<Trade, Quote>>(&mut m, &events);
        let lambda = m.value().expect("four returns");
        assert!((lambda - expected_lambda()).abs() < 1e-15);
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct FlowCandle {
        kyle_lambda: KyleLambda,
        kyle_lambda_stats: KyleLambdaStats,
    }

    #[test]
    fn empty_candles_have_no_flow() {
        let rule = TimeRule::new(MillisecondPeriod::from_non_zero(100));
        let mut a = GenericAggregator::<FlowCandle, _, Trade>::new(rule, false)
            .with_gaps(Gaps::Flat { max_candles: 3 });
        let mut bars = Vec::new();
        let trades = [
            trade(10, 100.0, 10.0),
            trade(20, 101.0, -20.0),
            trade(30, 102.0, 5.0),
            trade(350, 103.0, 10.0),
            trade(460, 104.0, 10.0),
        ];
        for t in &trades {
            a.update(t, &mut bars);
        }
        assert_eq!(bars.len(), 4);
        assert!(bars[0].candle().kyle_lambda().is_some());
        for flat in &bars[1..3] {
            let c = flat.candle();
            assert_eq!(flat.num_events(), 0);
            assert_eq!(c.kyle_lambda(), None);
            assert_eq!(c.kyle_lambda_stats(), RegressionSums::default());
        }
        // The trade at 350 has a return from the last trade before the gap.
        assert_eq!(bars[3].candle().kyle_lambda_stats().n, 1);
    }
}
