//! The values that a [`CusumRule`](crate::CusumRule) accumulates, read from each event,
//! and their standardization to units of standard deviations.

use crate::{
    candle_components::order_flow,
    price_source::{valid_quote, LogReturns},
    AsQuote, AsTrade, BestQuote, By, Error, PriceSource, Result, TakerTrade, TradePrice,
};

/// A value read from each event of a stream, e.g. the log return of a price,
/// that a rule such as the [`CusumRule`](crate::CusumRule) accumulates.
///
/// A signal follows the stream, not the candle: it keeps its state (e.g. the previous price
/// or its moving averages) when a candle closes, and rules do not reset it.
/// It may only use the events up to the current one, so that a candle never depends
/// on a later event (no lookahead).
pub trait Signal<E> {
    /// The value of the signal at `event`, `None` if the event carries no value of this signal,
    /// e.g. a quote for a signal of the trades, or the first price for a return.
    fn observe(&mut self, event: &E) -> Option<f64>;
}

/// The log return `ln(p / p_previous)` between consecutive prices of a [`PriceSource`],
/// the trade prices by default.
///
/// The first price has no return. The last price stays the reference when a candle closes,
/// so no return is lost at a candle boundary.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogReturn<P = TradePrice> {
    returns: LogReturns<P>,
}

impl<P> LogReturn<P> {
    /// Follow the prices of `source`, e.g. [`TradePrice`], [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice).
    pub fn new(source: P) -> Self {
        Self {
            returns: LogReturns::new(source),
        }
    }
}

impl<E, P: PriceSource<E>> Signal<E> for LogReturn<P> {
    #[inline(always)]
    fn observe(&mut self, event: &E) -> Option<f64> {
        self.returns.observe(event)?.log_return
    }
}

/// The z-score `(x - mean) / standard deviation` of an inner signal, with an exponentially
/// weighted moving mean and variance (the incremental form of Finch 2009),
/// so that its values are in units of standard deviations of the recent past.
///
/// A threshold on standardized values means the same for every symbol and every regime,
/// e.g. the threshold of a [`CusumRule`](crate::CusumRule) becomes a number of standard deviations.
/// Removing the mean also keeps a signal with a drift from adding up by itself.
///
/// There is no lookahead: each value is standardized with the mean and the variance
/// of the values before it, and only then updates them.
/// The first `warmup` values only feed the statistics and give `None`.
/// As long as `1 / n > alpha` for the `n`-th value, the statistics weigh every value equally
/// (the plain mean and the population variance), so that a small `alpha` does not start
/// from a variance near zero, which would give huge first z-scores.
/// While the variance is zero (e.g. a constant signal), the scale is unknown and the value
/// gives `None`; so does a z-score that overflows.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Standardized<S> {
    inner: S,
    warmup: u64,
    stats: EwStats,
}

impl<S> Standardized<S> {
    /// Standardize the values of `inner`.
    ///
    /// # Arguments:
    /// `inner`: The signal to standardize, e.g. a [`LogReturn`].
    /// `alpha`: The weight of the newest value in the moving mean and variance, in `(0, 1)`;
    ///     about `2 / (n + 1)` for a memory of `n` values.
    /// `warmup`: The number of values that only feed the statistics, at least 2.
    ///     About `1 / alpha` makes the first z-scores as reliable as the later ones.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `alpha` is not in `(0, 1)` or `warmup` is less than 2.
    /// With `alpha = 1` the variance would always be zero, so no value would ever be standardized.
    pub fn new(inner: S, alpha: f64, warmup: u64) -> Result<Self> {
        if !(alpha > 0.0 && alpha < 1.0) {
            return Err(Error::InvalidParam);
        }
        if warmup < 2 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            inner,
            warmup,
            stats: EwStats::new(alpha),
        })
    }
}

impl<E, S: Signal<E>> Signal<E> for Standardized<S> {
    fn observe(&mut self, event: &E) -> Option<f64> {
        let x = self.inner.observe(event)?;
        assert!(x.is_finite(), "signal values must be finite, got {x}");
        let z = if self.stats.count >= self.warmup {
            self.stats.z_score(x)
        } else {
            None
        };
        self.stats.update(x);
        debug_assert!(self.stats.count > 0);
        z
    }
}

/// The signed size of each trade in the currency of `by`: positive for a taker buy,
/// negative for a taker sell, as summed by the [`SignedVolumeRule`](crate::SignedVolumeRule).
///
/// Its scale is that of the trade sizes of the symbol, and its mean is rarely zero because one
/// side is usually more active, so wrap it in a [`Standardized`] signal for a
/// [`CusumRule`](crate::CusumRule). Trade sizes are heavy tailed:
/// a single large trade can then fire the filter on its own.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SignedVolume {
    by: By,
}

impl SignedVolume {
    /// The signed size of each trade in the currency `by`, as for the [`VolumeRule`](crate::VolumeRule).
    pub fn new(by: By) -> Self {
        Self { by }
    }
}

impl<E: AsTrade> Signal<E> for SignedVolume {
    #[inline(always)]
    fn observe(&mut self, event: &E) -> Option<f64> {
        let trade = event.as_trade()?;
        debug_assert!(trade.price() > 0.0, "prices must be positive");
        let size = self.by.size(trade);
        debug_assert!(size.is_finite(), "sizes must be finite");
        Some(size)
    }
}

/// The order flow imbalance increment of each quote update (Cont, Kukanov and Stoikov 2014):
/// `1{bid >= bid'} bid_size - 1{bid <= bid'} bid_size' - 1{ask <= ask'} ask_size + 1{ask >= ask'} ask_size'`,
/// where the primed values are those of the previous quote, in the unit of the quote sizes.
///
/// It is the size added to the bid minus the size added to the ask: positive when buyers
/// join the bid or sellers leave the ask. Its sum over an interval, the
/// [`OrderFlowImbalance`](crate::candle_components::OrderFlowImbalance), explains much of the
/// change of the mid price over that interval, with a slope inversely proportional to the depth.
///
/// The first quote has no increment. A quote with an invalid bid or ask
/// (see [`MidPrice`](crate::MidPrice)) or with a negative or non-finite size gives `None`
/// and is skipped: the next increment is measured from the last valid quote.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuoteOfi {
    /// The bid price, bid size, ask price and ask size of the last valid quote.
    previous: Option<[f64; 4]>,
}

/// The order flow imbalance increment of a quote update and the depth before it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OfiStep {
    /// The increment, in the unit of the quote sizes.
    pub(crate) increment: f64,
    /// The mean size at the best bid and ask of the previous quote, `(bid_size + ask_size) / 2`.
    pub(crate) depth: f64,
}

impl QuoteOfi {
    /// Start without a previous quote.
    pub fn new() -> Self {
        Self::default()
    }

    /// The increment of `event` and the depth of the quote before it,
    /// `None` for events other than valid quotes and for the first valid quote.
    pub(crate) fn step<E: AsQuote>(&mut self, event: &E) -> Option<OfiStep> {
        let current = quote_levels(event.as_quote()?)?;
        let previous = self.previous.replace(current)?;
        let increment = order_flow(current, previous);
        let depth = (previous[1] + previous[3]) / 2.0;
        debug_assert!(increment.is_finite());
        debug_assert!(depth >= 0.0, "sizes are not negative");
        Some(OfiStep { increment, depth })
    }
}

impl<E: AsQuote> Signal<E> for QuoteOfi {
    #[inline(always)]
    fn observe(&mut self, event: &E) -> Option<f64> {
        self.step(event).map(|step| step.increment)
    }
}

/// The bid price, bid size, ask price and ask size of `quote`,
/// `None` if its prices are invalid or a size is negative or not finite.
fn quote_levels<Q: BestQuote>(quote: &Q) -> Option<[f64; 4]> {
    let (bid_size, ask_size) = (quote.bid_size(), quote.ask_size());
    let valid_size = |size: f64| size.is_finite() && size >= 0.0;
    if !valid_quote(quote) || !valid_size(bid_size) || !valid_size(ask_size) {
        return None;
    }
    let levels = [quote.bid_price(), bid_size, quote.ask_price(), ask_size];
    debug_assert!(levels.iter().all(|level| level.is_finite()));
    Some(levels)
}

/// The natural log of the time between consecutive trades, in milliseconds
/// whatever the [`TIMESTAMP_RESOLUTION`](crate::Timestamped::TIMESTAMP_RESOLUTION) of the events.
///
/// Durations between trades are clustered and heavy tailed (Engle and Russell 1998);
/// their log is closer to normal and turns a change of the trading rate into a shift:
/// trading ten times faster lowers the value by `ln 10`, whatever the usual rate of the symbol.
/// A [`CusumRule`](crate::CusumRule) on the [`Standardized`] signal thus closes a candle when the
/// trading rate has changed, its down sum firing on an acceleration and its up sum on a slowdown.
///
/// A trade with the same timestamp as the previous trade gives `None`: such trades are
/// usually the fills of one taker order against several resting orders, so they arrive together,
/// and the log of zero is not finite. The next trade with a later timestamp measures from them.
/// Adding one timestamp unit instead would make the value depend on the resolution of the feed:
/// `ln(1 ms) = 0` but `ln(1 ns) = -13.8`. Within the resolution of a feed, arrivals still merge,
/// e.g. two orders in the same millisecond of a millisecond feed.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LogInterArrival {
    last_timestamp: Option<i64>,
}

impl LogInterArrival {
    /// Start without a previous trade.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<E: AsTrade> Signal<E> for LogInterArrival {
    fn observe(&mut self, event: &E) -> Option<f64> {
        event.as_trade()?;
        let timestamp = event.timestamp();
        let last = self.last_timestamp.replace(timestamp)?;
        let elapsed = timestamp.saturating_sub(last);
        assert!(
            elapsed >= 0,
            "trades in time order: {timestamp} after {last}"
        );
        if elapsed == 0 {
            return None;
        }
        let units_per_ms = E::TIMESTAMP_RESOLUTION.units_per_millisecond();
        let value = (elapsed as f64 / units_per_ms as f64).ln();
        debug_assert!(value.is_finite());
        Some(value)
    }
}

/// An exponentially weighted moving mean and variance whose weight is at least `1 / n`
/// for the `n`-th value, so that it starts as the plain mean and population variance.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct EwStats {
    alpha: f64,
    count: u64,
    mean: f64,
    variance: f64,
}

impl EwStats {
    fn new(alpha: f64) -> Self {
        assert!(alpha > 0.0, "alpha must be positive");
        assert!(alpha < 1.0, "alpha must be below 1");
        Self {
            alpha,
            count: 0,
            mean: 0.0,
            variance: 0.0,
        }
    }

    /// The z-score of `x` against the values so far, `None` if their variance is zero.
    fn z_score(&self, x: f64) -> Option<f64> {
        debug_assert!(self.count > 0, "the statistics need a value");
        debug_assert!(self.variance >= 0.0);
        if self.variance <= 0.0 {
            return None;
        }
        let z = (x - self.mean) / self.variance.sqrt();
        z.is_finite().then_some(z)
    }

    fn update(&mut self, x: f64) {
        debug_assert!(x.is_finite());
        self.count = self.count.saturating_add(1);
        // The weight only matters while `1 / n > alpha`, i.e. for small counts.
        let n = f64::from(u32::try_from(self.count).unwrap_or(u32::MAX));
        let weight = self.alpha.max(1.0 / n);
        let diff = x - self.mean;
        let increment = weight * diff;
        self.mean += increment;
        self.variance = (1.0 - weight) * (self.variance + diff * increment);
        debug_assert!(self.variance >= 0.0, "a sum of non-negative terms");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{
            random_flow, trade, trigger_indices, MicroTrade, NanoTrade,
        },
        CusumRule, MarketEvent, MidPrice, Quote, Trade,
    };

    /// A signal whose events are its values.
    #[derive(Debug, Clone)]
    struct Values;

    impl Signal<f64> for Values {
        fn observe(&mut self, event: &f64) -> Option<f64> {
            Some(*event)
        }
    }

    fn standardized(alpha: f64, warmup: u64) -> Standardized<Values> {
        Standardized::new(Values, alpha, warmup).expect("valid parameters")
    }

    fn observe_all<S: Signal<f64>>(signal: &mut S, values: &[f64]) -> Vec<Option<f64>> {
        values.iter().map(|v| signal.observe(v)).collect()
    }

    #[test]
    fn log_returns_of_the_trades() {
        let mut r: LogReturn = LogReturn::default();
        let trade = |price| Trade {
            timestamp: 0,
            price,
            size: 1.0,
        };
        assert_eq!(r.observe(&trade(100.0)), None);
        assert_eq!(r.observe(&trade(110.0)), Some((1.1_f64).ln()));
        assert_eq!(r.observe(&trade(110.0)), Some(0.0));
    }

    #[test]
    fn log_returns_of_the_mid_price_skip_trades() {
        let mut r = LogReturn::new(MidPrice);
        let quote = |bid, ask| {
            MarketEvent::<Trade, Quote>::Quote(Quote {
                timestamp: 0,
                bid_price: bid,
                bid_size: 1.0,
                ask_price: ask,
                ask_size: 1.0,
            })
        };
        let trade = MarketEvent::<Trade, Quote>::Trade(Trade {
            timestamp: 0,
            price: 500.0,
            size: 1.0,
        });
        assert_eq!(r.observe(&quote(99.0, 101.0)), None);
        assert_eq!(r.observe(&trade), None);
        assert_eq!(r.observe(&quote(0.0, 101.0)), None, "an invalid quote");
        assert_eq!(r.observe(&quote(109.0, 111.0)), Some((1.1_f64).ln()));
    }

    #[test]
    fn standardized_rejects_invalid_parameters() {
        assert!(Standardized::new(Values, 0.0, 10).is_err());
        assert!(Standardized::new(Values, -0.1, 10).is_err());
        assert!(Standardized::new(Values, 1.0, 10).is_err());
        assert!(Standardized::new(Values, f64::NAN, 10).is_err());
        assert!(Standardized::new(Values, 0.1, 0).is_err());
        assert!(Standardized::new(Values, 0.1, 1).is_err());
        assert!(Standardized::new(Values, 0.1, 2).is_ok());
    }

    #[test]
    fn standardized_uses_only_the_values_before() {
        // 1 and 3 warm up: mean 2, population variance 1.
        // 4 is 2 standard deviations above; then (alpha 0.5) mean 3, variance 0.5 * (1 + 2 * 1) = 1.5.
        // 3 is at the mean.
        let z = observe_all(&mut standardized(0.5, 2), &[1.0, 3.0, 4.0, 3.0]);
        assert_eq!(z, vec![None, None, Some(2.0), Some(0.0)]);
    }

    #[test]
    fn standardized_starts_with_equal_weights() {
        // With alpha 0.01, the first values weigh 1/n: after 1, 2, 3 the mean is 2
        // and the population variance 2/3, as without exponential weights.
        let mut s = standardized(0.01, 3);
        let z = observe_all(&mut s, &[1.0, 2.0, 3.0, 2.0 + (2.0_f64 / 3.0).sqrt()]);
        assert_eq!(z[..3], [None, None, None]);
        let last = z[3].expect("after the warm-up");
        assert!((last - 1.0).abs() < 1e-12, "{last}");
    }

    #[test]
    fn standardized_skips_a_zero_variance() {
        let mut s = standardized(0.5, 2);
        // Constant values: the scale is unknown, even for a different value.
        assert_eq!(observe_all(&mut s, &[5.0, 5.0, 5.0, 7.0]), vec![None; 4]);
        // 7 updated the statistics: mean 6, variance 0.5 * (0 + 2 * 1) = 1.
        assert_eq!(s.observe(&6.0), Some(0.0));
    }

    #[test]
    #[should_panic(expected = "finite")]
    fn standardized_rejects_a_non_finite_value() {
        let _ = observe_all(&mut standardized(0.5, 2), &[1.0, f64::NAN]);
    }

    #[test]
    fn standardized_values_do_not_depend_on_the_scale() {
        let values: Vec<f64> = (0..200).map(|i| f64::from(i % 7) - 2.5).collect();
        let scaled: Vec<f64> = values.iter().map(|v| 1_000.0 * v).collect();
        let a = observe_all(&mut standardized(0.05, 20), &values);
        let b = observe_all(&mut standardized(0.05, 20), &scaled);
        assert_eq!(a.iter().filter(|z| z.is_some()).count(), 180);
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(a.is_some(), b.is_some());
            if let (Some(a), Some(b)) = (a, b) {
                assert!((a - b).abs() < 1e-9, "{a} != {b}");
            }
        }
    }

    #[test]
    fn standardized_passes_events_without_a_value() {
        let mut s = Standardized::new(LogReturn::new(MidPrice), 0.5, 2).expect("valid parameters");
        let trade = MarketEvent::<Trade, Quote>::Trade(Trade {
            timestamp: 0,
            price: 100.0,
            size: 1.0,
        });
        for _ in 0..10 {
            assert_eq!(s.observe(&trade), None);
        }
        assert_eq!(s.stats.count, 0, "events without a value do not count");
    }

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

    fn trades_at(timestamps: &[i64], sizes: impl Fn(usize) -> f64) -> Vec<Trade> {
        timestamps
            .iter()
            .enumerate()
            .map(|(i, ts)| trade(*ts, 100.0, sizes(i)))
            .collect()
    }

    #[test]
    fn signed_volume_in_base_and_quote() {
        let buy = Event::Trade(trade(0, 50.0, 100.0));
        let sell = Event::Trade(trade(0, 50.0, -100.0));
        let mut quote_volume = SignedVolume::new(By::Quote);
        let mut base_volume = SignedVolume::new(By::Base);
        assert_eq!(quote_volume.observe(&buy), Some(100.0));
        assert_eq!(quote_volume.observe(&sell), Some(-100.0));
        assert_eq!(base_volume.observe(&sell), Some(-2.0));
        assert_eq!(base_volume.observe(&quote(99.0, 1.0, 101.0, 1.0)), None);
        assert_eq!(base_volume.observe(&trade(0, 50.0, 0.0)), Some(0.0));
    }

    /// The indices of the trades on which a CUSUM filter on `signal` fires.
    fn cusum_fires<S: Signal<Trade>>(signal: S, threshold: f64, trades: &[Trade]) -> Vec<usize> {
        let rule = CusumRule::new(threshold).expect("valid threshold");
        trigger_indices(&mut rule.with_signal(signal), trades)
    }

    fn standardized_volume() -> Standardized<SignedVolume> {
        Standardized::new(SignedVolume::new(By::Quote), 0.01, 100).expect("valid parameters")
    }

    #[test]
    fn a_cusum_on_signed_volume_with_a_bias_fires_regularly() {
        // Two buys and a sell, repeated: a buy bias of 1/3 per trade.
        let timestamps: Vec<i64> = (0..30).collect();
        let trades = trades_at(&timestamps, |i| if i % 3 == 2 { -1.0 } else { 1.0 });
        // The up sum: 1, 2, 1, 2, 3 (fires), then 0, 1, 2, 1, 2, 3 (fires), ...
        let fired = cusum_fires(SignedVolume::new(By::Quote), 3.0, &trades);
        assert_eq!(fired, vec![4, 10, 16, 22, 28]);
    }

    #[test]
    fn a_cusum_on_standardized_signed_volume_ignores_a_steady_bias() {
        let fair = random_flow(10_000, 0.5, 7);
        let biased = random_flow(10_000, 0.7, 7);
        let raw_fair = cusum_fires(SignedVolume::new(By::Quote), 10.0, &fair).len();
        let raw_biased = cusum_fires(SignedVolume::new(By::Quote), 10.0, &biased).len();
        assert!(raw_biased > 2 * raw_fair, "{raw_biased} vs {raw_fair}");
        // Without the mean, both flows are sequences of unit variance and fire about as often.
        let sd_fair = cusum_fires(standardized_volume(), 10.0, &fair).len();
        let sd_biased = cusum_fires(standardized_volume(), 10.0, &biased).len();
        assert!(sd_fair > 100);
        let ratio = sd_biased as f64 / sd_fair as f64;
        assert!((ratio - 1.0).abs() < 0.2, "{sd_biased} vs {sd_fair}");
        // A change of the flow still fires: ten sells, each about 1.5 standard deviations.
        let mut turned = biased;
        turned.extend((10_000..10_010).map(|ts| trade(ts, 100.0, -1.0)));
        let fired = cusum_fires(standardized_volume(), 4.0, &turned);
        assert!(fired.iter().filter(|i| **i >= 10_000).count() >= 2);
    }

    #[test]
    fn quote_ofi_follows_the_queues() {
        let mut ofi = QuoteOfi::new();
        assert_eq!(ofi.observe(&Event::Trade(trade(0, 100.0, 1.0))), None);
        assert_eq!(ofi.observe(&quote(100.0, 2.0, 101.0, 3.0)), None, "first");
        // Same prices: the bid queue grows by 3 and the ask queue shrinks by 2.
        assert_eq!(ofi.observe(&quote(100.0, 5.0, 101.0, 1.0)), Some(5.0));
        // A higher bid adds its whole size, the unchanged ask queue adds nothing.
        assert_eq!(ofi.observe(&quote(100.5, 1.0, 101.0, 1.0)), Some(1.0));
        // Invalid quotes are skipped.
        assert_eq!(ofi.observe(&quote(0.0, 1.0, 101.0, 1.0)), None);
        assert_eq!(ofi.observe(&quote(100.5, -1.0, 101.0, 1.0)), None);
        assert_eq!(ofi.observe(&quote(100.5, f64::NAN, 101.0, 1.0)), None);
        // A lower ask subtracts its whole size, measured from the last valid quote.
        assert_eq!(ofi.observe(&quote(100.5, 1.0, 100.8, 4.0)), Some(-4.0));
    }

    #[test]
    fn log_inter_arrival_in_milliseconds() {
        let mut ms = LogInterArrival::new();
        let values: Vec<Option<f64>> = [0, 10, 10, 1_010]
            .iter()
            .map(|ts| ms.observe(&trade(*ts, 100.0, 1.0)))
            .collect();
        let expected = vec![None, Some(10_f64.ln()), None, Some(1_000_f64.ln())];
        assert_eq!(values, expected, "a trade at the same time is skipped");
        let mut us = LogInterArrival::new();
        assert_eq!(us.observe(&MicroTrade(trade(0, 100.0, 1.0))), None);
        assert_eq!(
            us.observe(&MicroTrade(trade(10_000, 100.0, 1.0))),
            Some(10_f64.ln())
        );
        let mut ns = LogInterArrival::new();
        assert_eq!(ns.observe(&NanoTrade(trade(0, 100.0, 1.0))), None);
        assert_eq!(
            ns.observe(&NanoTrade(trade(10_000_000, 100.0, 1.0))),
            Some(10_f64.ln())
        );
        assert_eq!(
            ns.observe(&NanoTrade(trade(10_000_001, 100.0, 1.0))),
            Some(1e-6_f64.ln())
        );
    }

    #[test]
    fn log_inter_arrival_ignores_quotes() {
        let mut s = LogInterArrival::new();
        let late_quote = Event::Quote(Quote {
            timestamp: 5,
            ..Quote::default()
        });
        assert_eq!(s.observe(&Event::Trade(trade(0, 100.0, 1.0))), None);
        assert_eq!(s.observe(&late_quote), None);
        assert_eq!(
            s.observe(&Event::Trade(trade(20, 100.0, 1.0))),
            Some(20_f64.ln())
        );
    }

    #[test]
    #[should_panic(expected = "time order")]
    fn log_inter_arrival_rejects_trades_out_of_order() {
        let mut s = LogInterArrival::new();
        let _ = s.observe(&trade(10, 100.0, 1.0));
        let _ = s.observe(&trade(5, 100.0, 1.0));
    }

    #[test]
    fn a_cusum_on_standardized_inter_arrivals_fires_when_trading_speeds_up() {
        // 100 trades 0.8 or 1.25 seconds apart, then trades 10 ms apart.
        let mut timestamps = vec![0_i64];
        for i in 1..120 {
            let gap = match i {
                100.. => 10,
                _ if i % 2 == 0 => 800,
                _ => 1_250,
            };
            timestamps.push(timestamps[i - 1] + gap);
        }
        let trades = trades_at(&timestamps, |_| 1.0);
        let signal = Standardized::new(LogInterArrival::new(), 0.05, 20).expect("valid");
        let mut rule = CusumRule::new(3.0)
            .expect("valid threshold")
            .with_signal(signal);
        assert_eq!(trigger_indices(&mut rule, &trades).first(), Some(&100));
    }
}
