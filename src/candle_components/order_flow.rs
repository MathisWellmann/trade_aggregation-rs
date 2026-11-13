//! Components of the signed order flow of the taker trades: the price impact of the flow
//! (Kyle's lambda), the average price paid by each side, and the net flow with its extremes.
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

/// The notional and the base volume of the trades of one side, for its volume weighted price.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct SideVwap {
    quote: f64,
    base: f64,
}

impl SideVwap {
    #[inline(always)]
    fn add<T: TakerTrade>(&mut self, trade: &T) {
        self.quote += trade.quote_size().abs();
        self.base += trade.base_size().abs();
        debug_assert!(self.quote >= 0.0);
        debug_assert!(self.base >= 0.0);
    }

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        (self.base > 0.0).then(|| self.quote / self.base)
    }
}

/// The volume weighted average price of the taker buys, weighted by the base size:
/// their notional divided by their base volume. `None` without buys.
///
/// The taker buys fill at the ask and the sells at the bid, so the gap between the
/// [`BuyVwap`] and the [`SellVwap`] estimates the spread that takers pay without quote data,
/// and its sign shows which side pushed the price. Divide the gap by either of them
/// for a scale-free value.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BuyVwap {
    vwap: SideVwap,
}

impl CandleComponent for BuyVwap {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.vwap.value()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.vwap = SideVwap::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for BuyVwap {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if trade.size() > 0.0 {
            self.vwap.add(trade);
        }
    }
}

/// The volume weighted average price of the taker sells, weighted by the base size:
/// their notional divided by their base volume. `None` without sells.
/// See [`BuyVwap`] for the use of the gap between the two.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SellVwap {
    vwap: SideVwap,
}

impl CandleComponent for SellVwap {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        self.vwap.value()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.vwap = SideVwap::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for SellVwap {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if trade.size() < 0.0 {
            self.vwap.add(trade);
        }
    }
}

/// The net taker flow: the sum of the signed base sizes, buys minus sells,
/// also called the delta of the candle. 0 for a candle without trades.
///
/// Additive. Persistent net buying pushes the price up (order flow imbalance);
/// divide by the [`BaseVolume`](super::BaseVolume) for a scale-free imbalance in `[-1, 1]`.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetFlow {
    flow: f64,
}

impl CandleComponent for NetFlow {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.flow
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.flow = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for NetFlow {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.flow += trade.base_size();
    }
}

/// The highest running net flow of the candle: the maximum of the sum of the signed
/// base sizes from the open, which starts at 0, so the value is at least 0.
///
/// The "delta high" of a footprint chart: with [`NetFlow`] and [`NetFlowLow`] it shows
/// whether the buyers pushed and then gave up (a high far above the close) or kept control.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetFlowHigh {
    flow: f64,
    high: f64,
}

impl CandleComponent for NetFlowHigh {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.high
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.flow = 0.0;
        self.high = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for NetFlowHigh {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.flow += trade.base_size();
        self.high = self.high.max(self.flow);
        debug_assert!(self.high >= 0.0);
        debug_assert!(self.high >= self.flow);
    }
}

/// The lowest running net flow of the candle: the minimum of the sum of the signed
/// base sizes from the open, which starts at 0, so the value is at most 0.
///
/// The "delta low" of a footprint chart, the counterpart of [`NetFlowHigh`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetFlowLow {
    flow: f64,
    low: f64,
}

impl CandleComponent for NetFlowLow {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.low
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.flow = 0.0;
        self.low = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for NetFlowLow {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        self.flow += trade.base_size();
        self.low = self.low.min(self.flow);
        debug_assert!(self.low <= 0.0);
        debug_assert!(self.low <= self.flow);
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

    #[test]
    fn side_vwaps_weight_the_price_by_base_size() {
        // Inverse: 100 USD at 100 is 1 BTC, 400 USD at 200 is 2 BTC, so (100 + 400) / 3.
        let inverse = [
            trade(0, 100.0, 100.0),
            trade(1, 200.0, 400.0),
            trade(2, 50.0, -50.0),
            trade(3, 70.0, 0.0),
        ];
        let (mut buys, mut sells) = (BuyVwap::default(), SellVwap::default());
        update_all(&mut buys, &inverse);
        update_all(&mut sells, &inverse);
        assert!((buys.value().expect("buys") - 500.0 / 3.0).abs() < 1e-12);
        assert_eq!(sells.value(), Some(50.0));
        buys.reset();
        sells.reset();
        assert_eq!(buys.value(), None);
        assert_eq!(sells.value(), None);

        let spot = [
            Spot {
                price: 100.0,
                size: 1.0,
            },
            Spot {
                price: 200.0,
                size: 3.0,
            },
            Spot {
                price: 90.0,
                size: -2.0,
            },
            Spot {
                price: 80.0,
                size: -2.0,
            },
        ];
        update_all(&mut buys, &spot);
        update_all(&mut sells, &spot);
        assert_eq!(buys.value(), Some(175.0));
        assert_eq!(sells.value(), Some(85.0));
    }

    #[test]
    fn net_flow_tracks_the_running_delta_from_the_open() {
        let spot = |size| Spot { price: 10.0, size };
        let flows = [spot(2.0), spot(-5.0), spot(0.0), spot(1.0)];
        let (mut net, mut high, mut low) = (
            NetFlow::default(),
            NetFlowHigh::default(),
            NetFlowLow::default(),
        );
        update_all(&mut net, &flows);
        update_all(&mut high, &flows);
        update_all(&mut low, &flows);
        // Running: 2, -3, -3, -2.
        assert_eq!(net.value(), -2.0);
        assert_eq!(high.value(), 2.0);
        assert_eq!(low.value(), -3.0);
        net.reset();
        high.reset();
        low.reset();
        assert_eq!((net.value(), high.value(), low.value()), (0.0, 0.0, 0.0));
        // The open counts as 0, so a candle of sells has a high of 0.
        update_all(&mut net, &[spot(-1.0)]);
        update_all(&mut high, &[spot(-1.0)]);
        update_all(&mut low, &[spot(-1.0)]);
        assert_eq!((net.value(), high.value(), low.value()), (-1.0, 0.0, -1.0));

        // Inverse trades: 200 USD at 100 is 2 BTC.
        let mut inverse = NetFlow::default();
        update_all(
            &mut inverse,
            &[trade(0, 100.0, 200.0), trade(1, 50.0, -50.0)],
        );
        assert_eq!(inverse.value(), 1.0);
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct FlowCandle {
        kyle_lambda: KyleLambda,
        kyle_lambda_stats: KyleLambdaStats,
        buy_vwap: BuyVwap,
        sell_vwap: SellVwap,
        net_flow: NetFlow,
        net_flow_high: NetFlowHigh,
        net_flow_low: NetFlowLow,
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
            assert_eq!(c.buy_vwap(), None);
            assert_eq!(c.sell_vwap(), None);
            assert_eq!(c.net_flow(), 0.0);
            assert_eq!(c.net_flow_high(), 0.0);
            assert_eq!(c.net_flow_low(), 0.0);
        }
        // The trade at 350 has a return from the last trade before the gap.
        assert_eq!(bars[3].candle().kyle_lambda_stats().n, 1);
    }
}
