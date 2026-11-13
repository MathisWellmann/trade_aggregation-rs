//! Components from the quotes (best bid and ask) of a candle.
//!
//! They act on events that are or contain quotes ([`AsQuote`]),
//! e.g. a [`MarketEvent`](crate::MarketEvent) of trades and quotes, and ignore other events.
//! A value is `None` while no quote has arrived.
//!
//! The quote in effect when a candle opens is the last quote before it:
//! the components keep it over a reset, so a candle without quote updates
//! still has the spread and the mid price of the market.

use super::RealizedVariance;
use crate::{
    price_source::valid_quote, AsQuote, AsTrade, BestQuote, CandleComponent, CandleComponentUpdate,
    Microprice, TakerTrade,
};

/// The mid price when the candle opened: the last mid price before it, or the first one.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidOpen {
    open: Option<f64>,
    last: Option<f64>,
}

impl CandleComponent for MidOpen {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.open
    }

    fn reset(&mut self) {
        self.open = self.last;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for MidOpen {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        let mid = quote.mid_price();
        self.open.get_or_insert(mid);
        self.last = Some(mid);
    }
}

/// The highest mid price of the candle, including the one in effect when it opened.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidHigh {
    high: Option<f64>,
    last: Option<f64>,
}

impl CandleComponent for MidHigh {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.high
    }

    fn reset(&mut self) {
        self.high = self.last;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for MidHigh {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        let mid = quote.mid_price();
        self.high = Some(self.high.map_or(mid, |high| high.max(mid)));
        self.last = Some(mid);
    }
}

/// The lowest mid price of the candle, including the one in effect when it opened.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidLow {
    low: Option<f64>,
    last: Option<f64>,
}

impl CandleComponent for MidLow {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.low
    }

    fn reset(&mut self) {
        self.low = self.last;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for MidLow {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        let mid = quote.mid_price();
        self.low = Some(self.low.map_or(mid, |low| low.min(mid)));
        self.last = Some(mid);
    }
}

/// The mid price when the candle closed.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidClose {
    last: Option<f64>,
}

impl CandleComponent for MidClose {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn reset(&mut self) {}
}

impl<E: AsQuote> CandleComponentUpdate<E> for MidClose {
    fn update(&mut self, event: &E) {
        if let Some(quote) = event.as_quote() {
            self.last = Some(quote.mid_price());
        }
    }
}

/// The number of quote updates in the candle.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct QuoteUpdates {
    count: u64,
}

impl CandleComponent for QuoteUpdates {
    type Value = u64;

    fn value(&self) -> u64 {
        self.count
    }

    fn reset(&mut self) {
        self.count = 0;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for QuoteUpdates {
    fn update(&mut self, event: &E) {
        if event.as_quote().is_some() {
            self.count += 1;
        }
    }
}

/// The realized variance of the weighted average price (WAP), the [`Microprice`]:
/// the sum of the squared log returns between consecutive quotes,
/// starting at the last quote before the candle.
pub type WapRealizedVariance = RealizedVariance<Microprice>;

/// The average of a value that changes at quote updates, weighted by the time each value was in effect.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct TimeAverage {
    /// The integral of the value over the time covered in this candle.
    integral: f64,
    /// The time covered, in event units.
    covered: i64,
    /// The value in effect, and since when.
    current: Option<(f64, i64)>,
}

impl TimeAverage {
    /// Add the time up to `timestamp` with the current value.
    fn integrate_to(&mut self, timestamp: i64) {
        let Some((value, since)) = self.current else {
            return;
        };
        let elapsed = timestamp - since;
        assert!(elapsed >= 0, "events must arrive in time order");
        self.integral += value * elapsed as f64;
        self.covered += elapsed;
        self.current = Some((value, timestamp));
    }

    fn observe(&mut self, timestamp: i64, value: f64) {
        debug_assert!(value.is_finite());
        self.integrate_to(timestamp);
        self.current = Some((value, timestamp));
    }

    /// Keeps the current value, which is in effect when the next candle opens.
    fn reset(&mut self) {
        self.integral = 0.0;
        self.covered = 0;
    }

    /// The average over the covered time; the current value if no time is covered.
    fn value(&self) -> Option<f64> {
        if self.covered > 0 {
            return Some(self.integral / self.covered as f64);
        }
        self.current.map(|(value, _)| value)
    }
}

/// The relative spread `(ask - bid) / mid`, averaged over the time of the candle.
/// The time from the last quote to the close of the candle counts,
/// see [`CandleComponentUpdate::close_at`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeWeightedSpread {
    average: TimeAverage,
}

impl CandleComponent for TimeWeightedSpread {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.average.value()
    }

    fn reset(&mut self) {
        self.average.reset();
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for TimeWeightedSpread {
    fn update(&mut self, event: &E) {
        if let Some(quote) = event.as_quote() {
            let spread = (quote.ask_price() - quote.bid_price()) / quote.mid_price();
            self.average.observe(event.timestamp(), spread);
        }
    }

    fn close_at(&mut self, timestamp: i64) {
        self.average.integrate_to(timestamp);
    }
}

/// The level 1 imbalance `(bid size - ask size) / (bid size + ask size)` in `[-1, 1]`,
/// averaged over the time of the candle. Positive when buyers queue more than sellers.
/// A quote without sizes counts as balanced.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeWeightedImbalance {
    average: TimeAverage,
}

impl CandleComponent for TimeWeightedImbalance {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.average.value()
    }

    fn reset(&mut self) {
        self.average.reset();
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for TimeWeightedImbalance {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        let sizes = quote.bid_size() + quote.ask_size();
        let imbalance = if sizes > 0.0 {
            (quote.bid_size() - quote.ask_size()) / sizes
        } else {
            0.0
        };
        debug_assert!((-1.0..=1.0).contains(&imbalance));
        self.average.observe(event.timestamp(), imbalance);
    }

    fn close_at(&mut self, timestamp: i64) {
        self.average.integrate_to(timestamp);
    }
}

/// The order flow imbalance of Cont, Kukanov and Stoikov (2014):
/// the net size added on the bid minus the net size added on the ask, summed over the quote updates,
/// in the unit of the quote sizes. A rising bid or a growing bid queue adds to it,
/// a falling ask or a growing ask queue subtracts.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrderFlowImbalance {
    sum: f64,
    /// The previous bid price, bid size, ask price and ask size.
    previous: Option<[f64; 4]>,
}

/// The contribution of one quote update to the order flow imbalance.
fn order_flow([bid, bid_size, ask, ask_size]: [f64; 4], previous: [f64; 4]) -> f64 {
    let [previous_bid, previous_bid_size, previous_ask, previous_ask_size] = previous;
    let mut flow = 0.0;
    if bid >= previous_bid {
        flow += bid_size;
    }
    if bid <= previous_bid {
        flow -= previous_bid_size;
    }
    if ask <= previous_ask {
        flow -= ask_size;
    }
    if ask >= previous_ask {
        flow += previous_ask_size;
    }
    flow
}

impl CandleComponent for OrderFlowImbalance {
    type Value = f64;

    fn value(&self) -> f64 {
        self.sum
    }

    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for OrderFlowImbalance {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        let current = [
            quote.bid_price(),
            quote.bid_size(),
            quote.ask_price(),
            quote.ask_size(),
        ];
        if let Some(previous) = self.previous {
            self.sum += order_flow(current, previous);
        }
        self.previous = Some(current);
    }
}

/// The effective spread `2 * |price - mid| / mid` of the trades, against the mid price of the
/// last quote before each trade, weighted by the base size of the trades.
/// `None` without a trade after a quote.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EffectiveSpread {
    weighted_sum: f64,
    weights: f64,
    mid: Option<f64>,
}

impl CandleComponent for EffectiveSpread {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        (self.weights > 0.0).then(|| self.weighted_sum / self.weights)
    }

    fn reset(&mut self) {
        self.weighted_sum = 0.0;
        self.weights = 0.0;
    }
}

impl<E: AsQuote + AsTrade> CandleComponentUpdate<E> for EffectiveSpread {
    fn update(&mut self, event: &E) {
        if let Some(quote) = event.as_quote() {
            self.mid = Some(quote.mid_price());
        }
        let (Some(trade), Some(mid)) = (event.as_trade(), self.mid) else {
            return;
        };
        let weight = trade.base_size().abs();
        self.weighted_sum += weight * 2.0 * (trade.price() - mid).abs() / mid;
        self.weights += weight;
    }
}

/// The relative spread `(ask - bid) / mid` of the last valid quote: the state the market is in
/// when the candle closes, which is where the next candle starts from, complementing the
/// time-weighted spread ([`TimeWeightedSpread`]) that describes the candle itself.
/// `None` while no valid quote has arrived. Invalid quotes (a bid or ask that is not positive and
/// finite, or a crossed book) are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SpreadAtClose {
    last: Option<f64>,
}

impl CandleComponent for SpreadAtClose {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn reset(&mut self) {}
}

impl<E: AsQuote> CandleComponentUpdate<E> for SpreadAtClose {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        self.last = Some((quote.ask_price() - quote.bid_price()) / quote.mid_price());
    }
}

/// The level 1 imbalance `(bid size - ask size) / (bid size + ask size)` of the last valid quote,
/// in `[-1, 1]`: the queue the next candle starts from, complementing the time-weighted imbalance
/// ([`TimeWeightedImbalance`]) that describes the candle itself. `0` when both sizes are zero.
/// `None` while no valid quote has arrived. Invalid quotes are skipped, see [`SpreadAtClose`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImbalanceAtClose {
    last: Option<f64>,
}

impl CandleComponent for ImbalanceAtClose {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn reset(&mut self) {}
}

impl<E: AsQuote> CandleComponentUpdate<E> for ImbalanceAtClose {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        let sizes = quote.bid_size() + quote.ask_size();
        let imbalance = if sizes > 0.0 {
            (quote.bid_size() - quote.ask_size()) / sizes
        } else {
            0.0
        };
        debug_assert!((-1.0..=1.0).contains(&imbalance));
        self.last = Some(imbalance);
    }
}

/// The offset of the microprice from the mid, `(microprice - mid) / mid`, of the last valid quote.
/// Stoikov (2018) shows the microprice leans to the side of the book that is likely to be taken
/// next, so the offset is a short-horizon drift signal for the start of the next candle.
/// `None` while no valid quote has arrived. Invalid quotes are skipped, see [`SpreadAtClose`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MicropriceOffsetAtClose {
    last: Option<f64>,
}

impl CandleComponent for MicropriceOffsetAtClose {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn reset(&mut self) {}
}

impl<E: AsQuote> CandleComponentUpdate<E> for MicropriceOffsetAtClose {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        let mid = quote.mid_price();
        self.last = Some((quote.weighted_price() - mid) / mid);
    }
}

/// The depth of the last valid quote, `bid size + ask size`, in the unit of the quote sizes:
/// the liquidity available at the touch when the candle closed. Cont, Kukanov and Stoikov (2014)
/// show the price impact of order flow is inversely proportional to depth, so it normalises
/// [`OrderFlowImbalance`] for the start of the next candle.
/// `None` while no valid quote has arrived. Invalid quotes are skipped, see [`SpreadAtClose`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DepthAtClose {
    last: Option<f64>,
}

impl CandleComponent for DepthAtClose {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.last
    }

    fn reset(&mut self) {}
}

impl<E: AsQuote> CandleComponentUpdate<E> for DepthAtClose {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        self.last = Some(quote.bid_size() + quote.ask_size());
    }
}

/// The depth `bid size + ask size`, in the unit of the quote sizes, averaged over the time of the
/// candle. The time from the last quote to the close of the candle counts,
/// see [`CandleComponentUpdate::close_at`]. Cont, Kukanov and Stoikov (2014) show the price impact
/// of order flow is inversely proportional to depth, so it normalises [`OrderFlowImbalance`].
/// Invalid quotes are skipped, see [`SpreadAtClose`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeWeightedDepth {
    average: TimeAverage,
}

impl CandleComponent for TimeWeightedDepth {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.average.value()
    }

    fn reset(&mut self) {
        self.average.reset();
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for TimeWeightedDepth {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        self.average
            .observe(event.timestamp(), quote.bid_size() + quote.ask_size());
    }

    fn close_at(&mut self, timestamp: i64) {
        self.average.integrate_to(timestamp);
    }
}

/// The largest relative spread `(ask - bid) / mid` of the candle, including the one in effect
/// when it opened: the widest the market was willing to trade at, a risk measure the average
/// ([`TimeWeightedSpread`]) does not capture. `None` while no valid quote has arrived.
/// Invalid quotes are skipped, see [`SpreadAtClose`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxSpread {
    max: Option<f64>,
    last: Option<f64>,
}

impl CandleComponent for MaxSpread {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.max
    }

    fn reset(&mut self) {
        self.max = self.last;
    }
}

impl<E: AsQuote> CandleComponentUpdate<E> for MaxSpread {
    fn update(&mut self, event: &E) {
        let Some(quote) = event.as_quote() else {
            return;
        };
        if !valid_quote(quote) {
            return;
        }
        let spread = (quote.ask_price() - quote.bid_price()) / quote.mid_price();
        self.max = Some(self.max.map_or(spread, |max| max.max(spread)));
        self.last = Some(spread);
    }
}

/// Relative tolerance for comparing a trade price to a quote price, so that equal decimal prices
/// parsed as `f64` (e.g. `"101.1"` from one feed and `"101.10"` from another) compare as equal
/// instead of differing in the last bit.
const PRICE_TOLERANCE: f64 = 1e-9;

/// Whether `price` and `reference` are equal within [`PRICE_TOLERANCE`] of `reference`.
fn approx_eq(price: f64, reference: f64) -> bool {
    (price - reference).abs() <= PRICE_TOLERANCE * reference.abs()
}

/// Where a trade executed against the quote that was prevailing (the last valid quote before it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TradeLocation {
    /// At the touch: a buy at the ask, or a sell at the bid.
    AtTouch,
    /// Strictly between the bid and the ask: price improvement or hidden liquidity.
    Inside,
    /// Through the touch: a buy above the ask or a sell below the bid, sweeping levels or hitting
    /// a stale quote.
    Through,
}

/// Classify a trade of `price` against the prevailing `bid` and `ask`.
///
/// `None` for a buy at or below the bid, or a sell at or above the ask (usually a stale quote),
/// which belong to none of the three locations. Prices are compared with [`PRICE_TOLERANCE`].
fn locate_trade(price: f64, bid: f64, ask: f64, is_buy: bool) -> Option<TradeLocation> {
    let (touch, far) = if is_buy { (ask, bid) } else { (bid, ask) };
    if approx_eq(price, touch) {
        return Some(TradeLocation::AtTouch);
    }
    let beyond_touch = if is_buy { price > touch } else { price < touch };
    if beyond_touch {
        return Some(TradeLocation::Through);
    }
    let beyond_far = if is_buy { price > far } else { price < far };
    if beyond_far && !approx_eq(price, far) {
        return Some(TradeLocation::Inside);
    }
    None
}

/// Shared state of the trade-location components: the prevailing quote (the last valid quote
/// before a trade, kept across [`CandleComponent::reset`] like [`EffectiveSpread`]'s mid) and the
/// running base volume at each [`TradeLocation`].
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct TradeLocations {
    bid: Option<f64>,
    ask: Option<f64>,
    at_touch: f64,
    inside: f64,
    through: f64,
    denominator: f64,
}

impl TradeLocations {
    fn update<E: AsQuote + AsTrade>(&mut self, event: &E) {
        if let Some(quote) = event.as_quote() {
            if valid_quote(quote) {
                self.bid = Some(quote.bid_price());
                self.ask = Some(quote.ask_price());
            }
        }
        let (Some(trade), Some(bid), Some(ask)) = (event.as_trade(), self.bid, self.ask) else {
            return;
        };
        let size = trade.base_size();
        if size == 0.0 {
            return;
        }
        let volume = size.abs();
        self.denominator += volume;
        match locate_trade(trade.price(), bid, ask, size > 0.0) {
            Some(TradeLocation::AtTouch) => self.at_touch += volume,
            Some(TradeLocation::Inside) => self.inside += volume,
            Some(TradeLocation::Through) => self.through += volume,
            None => {}
        }
    }

    fn reset(&mut self) {
        self.at_touch = 0.0;
        self.inside = 0.0;
        self.through = 0.0;
        self.denominator = 0.0;
    }

    /// The share of `numerator` of the base volume of trades that had a prevailing valid quote.
    fn share(&self, numerator: f64) -> Option<f64> {
        (self.denominator > 0.0).then(|| numerator / self.denominator)
    }
}

/// Share of the base volume of trades at the touch against the prevailing quote (the last valid
/// quote before each trade): a taker buy that pays the ask, or a taker sell that hits the bid.
/// The denominator is the base volume of trades that had a prevailing valid quote;
/// `None` without such a trade.
///
/// A trade on the wrong side (a buy at or below the bid, or a sell at or above the ask, usually
/// from a stale quote) belongs to none of [`VolumeAtTouch`], [`VolumeInsideSpread`] and
/// [`VolumeThroughTouch`], so the three can sum to less than 1. Prices are compared with a
/// relative tolerance of 1e-9 of the quote price. Trades of size zero are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeAtTouch {
    locations: TradeLocations,
}

impl CandleComponent for VolumeAtTouch {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.locations.share(self.locations.at_touch)
    }

    fn reset(&mut self) {
        self.locations.reset();
    }
}

impl<E: AsQuote + AsTrade> CandleComponentUpdate<E> for VolumeAtTouch {
    fn update(&mut self, event: &E) {
        self.locations.update(event);
    }
}

/// Share of the base volume of trades strictly inside the prevailing spread (the last valid quote
/// before each trade): price improvement or hidden liquidity between the bid and the ask.
/// The denominator is the base volume of trades that had a prevailing valid quote;
/// `None` without such a trade.
///
/// A trade on the wrong side (a buy at or below the bid, or a sell at or above the ask, usually
/// from a stale quote) belongs to none of [`VolumeAtTouch`], [`VolumeInsideSpread`] and
/// [`VolumeThroughTouch`], so the three can sum to less than 1. Prices are compared with a
/// relative tolerance of 1e-9 of the quote price. Trades of size zero are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeInsideSpread {
    locations: TradeLocations,
}

impl CandleComponent for VolumeInsideSpread {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.locations.share(self.locations.inside)
    }

    fn reset(&mut self) {
        self.locations.reset();
    }
}

impl<E: AsQuote + AsTrade> CandleComponentUpdate<E> for VolumeInsideSpread {
    fn update(&mut self, event: &E) {
        self.locations.update(event);
    }
}

/// Share of the base volume of trades through the prevailing touch (the last valid quote before
/// each trade): a buy above the ask or a sell below the bid, which swept several levels of the
/// book, or traded against a stale quote.
/// The denominator is the base volume of trades that had a prevailing valid quote;
/// `None` without such a trade.
///
/// A trade on the wrong side (a buy at or below the bid, or a sell at or above the ask, usually
/// from a stale quote) belongs to none of [`VolumeAtTouch`], [`VolumeInsideSpread`] and
/// [`VolumeThroughTouch`], so the three can sum to less than 1. Prices are compared with a
/// relative tolerance of 1e-9 of the quote price. Trades of size zero are skipped.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VolumeThroughTouch {
    locations: TradeLocations,
}

impl CandleComponent for VolumeThroughTouch {
    type Value = Option<f64>;

    fn value(&self) -> Option<f64> {
        self.locations.share(self.locations.through)
    }

    fn reset(&mut self) {
        self.locations.reset();
    }
}

impl<E: AsQuote + AsTrade> CandleComponentUpdate<E> for VolumeThroughTouch {
    fn update(&mut self, event: &E) {
        self.locations.update(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarketEvent, Quote, Trade};

    fn quote(timestamp: i64, bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> Quote {
        Quote {
            timestamp,
            bid_price: bid,
            bid_size,
            ask_price: ask,
            ask_size,
        }
    }

    /// Close the candle of a component that acts on quotes.
    fn close<C: CandleComponentUpdate<Quote>>(component: &mut C, timestamp: i64) {
        component.close_at(timestamp);
    }

    fn mid(timestamp: i64, mid: f64) -> Quote {
        quote(timestamp, mid - 1.0, 1.0, mid + 1.0, 1.0)
    }

    #[test]
    fn mid_ohlc_starts_at_the_mid_in_effect() {
        let (mut o, mut h, mut l, mut c) = (
            MidOpen::default(),
            MidHigh::default(),
            MidLow::default(),
            MidClose::default(),
        );
        assert_eq!(o.value(), None);
        let mut update = |q: &Quote| {
            o.update(q);
            h.update(q);
            l.update(q);
            c.update(q);
        };
        for (ts, m) in [(0, 100.0), (1, 102.0), (2, 99.0)] {
            update(&mid(ts, m));
        }
        assert_eq!(o.value(), Some(100.0));
        assert_eq!(h.value(), Some(102.0));
        assert_eq!(l.value(), Some(99.0));
        assert_eq!(c.value(), Some(99.0));
        o.reset();
        h.reset();
        l.reset();
        c.reset();
        // Without updates the candle is flat at the mid in effect.
        assert_eq!(
            (o.value(), h.value(), l.value(), c.value()),
            (Some(99.0), Some(99.0), Some(99.0), Some(99.0))
        );
        o.update(&mid(3, 101.0));
        h.update(&mid(3, 101.0));
        l.update(&mid(3, 101.0));
        assert_eq!(o.value(), Some(99.0));
        assert_eq!(h.value(), Some(101.0));
        assert_eq!(l.value(), Some(99.0));
    }

    #[test]
    fn quote_components_ignore_trades() {
        let mut updates = QuoteUpdates::default();
        let trade = MarketEvent::<Trade, Quote>::Trade(Trade::default());
        updates.update(&trade);
        updates.update(&MarketEvent::<Trade, Quote>::Quote(mid(0, 10.0)));
        assert_eq!(updates.value(), 1);
        updates.reset();
        assert_eq!(updates.value(), 0);
    }

    #[test]
    fn the_spread_is_weighted_by_time_up_to_the_close() {
        let mut s = TimeWeightedSpread::default();
        assert_eq!(s.value(), None);
        // A relative spread of 2 / 100 for 10 ms, then 4 / 100 for 30 ms.
        s.update(&mid(0, 100.0));
        s.update(&quote(10, 98.0, 1.0, 102.0, 1.0));
        close(&mut s, 40);
        let expected = (0.02 * 10.0 + 0.04 * 30.0) / 40.0;
        assert!((s.value().expect("quotes") - expected).abs() < 1e-15);
        // The next candle starts with the spread in effect.
        s.reset();
        close(&mut s, 50);
        assert!((s.value().expect("quotes") - 0.04).abs() < 1e-15);
    }

    #[test]
    fn the_imbalance_is_weighted_by_time() {
        let mut i = TimeWeightedImbalance::default();
        i.update(&quote(0, 99.0, 3.0, 101.0, 1.0));
        i.update(&quote(30, 99.0, 1.0, 101.0, 1.0));
        close(&mut i, 40);
        assert_eq!(i.value(), Some((0.5 * 30.0) / 40.0));
        i.reset();
        i.update(&quote(40, 99.0, 0.0, 101.0, 0.0));
        close(&mut i, 50);
        assert_eq!(i.value(), Some(0.0), "a quote without sizes is balanced");
    }

    #[test]
    fn order_flow_imbalance_follows_the_queues() {
        let mut ofi = OrderFlowImbalance::default();
        ofi.update(&quote(0, 99.0, 5.0, 101.0, 5.0));
        assert_eq!(ofi.value(), 0.0, "the first quote has no previous one");
        // The bid queue grows by 2: +2.
        ofi.update(&quote(1, 99.0, 7.0, 101.0, 5.0));
        assert_eq!(ofi.value(), 2.0);
        // The ask queue grows by 3: -3.
        ofi.update(&quote(2, 99.0, 7.0, 101.0, 8.0));
        assert_eq!(ofi.value(), -1.0);
        // The bid moves up with 4: +4. The ask moves up and its old queue of 8 left: +8.
        ofi.update(&quote(3, 100.0, 4.0, 102.0, 1.0));
        assert_eq!(ofi.value(), 11.0);
        ofi.reset();
        assert_eq!(ofi.value(), 0.0);
        // The bid drops, its old queue of 4 left: -4.
        ofi.update(&quote(4, 99.0, 9.0, 102.0, 1.0));
        assert_eq!(ofi.value(), -4.0);
    }

    #[test]
    fn wap_realized_variance_sums_squared_log_returns() {
        let mut rv = WapRealizedVariance::default();
        rv.update(&mid(0, 100.0));
        rv.update(&mid(1, 110.0));
        let r = (110.0_f64 / 100.0).ln();
        assert!((rv.value() - r * r).abs() < 1e-15);
        rv.reset();
        rv.update(&mid(2, 100.0));
        assert!(
            (rv.value() - r * r).abs() < 1e-15,
            "starts at the previous WAP"
        );
    }

    #[test]
    fn effective_spread_measures_trades_against_the_mid() {
        type Event = MarketEvent<Trade, Quote>;
        let trade = |price, size| {
            Event::Trade(Trade {
                timestamp: 1,
                price,
                size,
            })
        };
        let mut e = EffectiveSpread::default();
        e.update(&trade(100.0, 1.0));
        assert_eq!(e.value(), None, "no mid before the first quote");
        e.update(&Event::Quote(mid(0, 100.0)));
        // Base sizes 1 and 3 (quote sizes divided by the price).
        e.update(&trade(101.0, 101.0));
        e.update(&trade(98.0, -294.0));
        let expected = (1.0 * 0.02 + 3.0 * 0.04) / 4.0;
        assert!((e.value().expect("trades") - expected).abs() < 1e-15);
    }

    #[test]
    fn level1_state_at_close_reads_the_last_valid_quote() {
        let (mut spread, mut imbalance, mut offset, mut depth) = (
            SpreadAtClose::default(),
            ImbalanceAtClose::default(),
            MicropriceOffsetAtClose::default(),
            DepthAtClose::default(),
        );
        assert_eq!(spread.value(), None);
        assert_eq!(imbalance.value(), None);
        assert_eq!(offset.value(), None);
        assert_eq!(depth.value(), None);

        // Mid 100, three times more size on the bid: the microprice leans to the ask.
        let q = quote(0, 99.0, 3.0, 101.0, 1.0);
        spread.update(&q);
        imbalance.update(&q);
        offset.update(&q);
        depth.update(&q);
        assert_eq!(spread.value(), Some(0.02));
        assert_eq!(imbalance.value(), Some(0.5));
        assert_eq!(offset.value(), Some(0.005));
        assert_eq!(depth.value(), Some(4.0));

        // An invalid (crossed) quote is skipped: the state stays at the last valid quote.
        let crossed = quote(1, 102.0, 1.0, 100.0, 1.0);
        spread.update(&crossed);
        depth.update(&crossed);
        assert_eq!(spread.value(), Some(0.02));
        assert_eq!(depth.value(), Some(4.0));

        // The state is kept over a reset: a candle without quote updates still has it.
        spread.reset();
        imbalance.reset();
        offset.reset();
        depth.reset();
        assert_eq!(spread.value(), Some(0.02));

        let q = quote(2, 100.0, 2.0, 102.0, 2.0);
        spread.update(&q);
        imbalance.update(&q);
        offset.update(&q);
        depth.update(&q);
        assert_eq!(spread.value(), Some(2.0 / 101.0));
        assert_eq!(imbalance.value(), Some(0.0));
        assert_eq!(offset.value(), Some(0.0));
        assert_eq!(depth.value(), Some(4.0));
    }

    #[test]
    fn time_weighted_depth_is_weighted_by_time_up_to_the_close() {
        let mut d = TimeWeightedDepth::default();
        assert_eq!(d.value(), None);
        // A depth of 4 for 10 ms, then 6 for 30 ms.
        d.update(&quote(0, 99.0, 2.0, 101.0, 2.0));
        d.update(&quote(10, 98.0, 1.0, 102.0, 5.0));
        close(&mut d, 40);
        let expected = (4.0 * 10.0 + 6.0 * 30.0) / 40.0;
        assert!((d.value().expect("quotes") - expected).abs() < 1e-15);
        // The next candle starts with the depth in effect.
        d.reset();
        close(&mut d, 50);
        assert!((d.value().expect("quotes") - 6.0).abs() < 1e-15);
        // An invalid quote does not change the depth in effect.
        d.update(&quote(60, 0.0, 100.0, 101.0, 1.0));
        close(&mut d, 70);
        assert!((d.value().expect("quotes") - 6.0).abs() < 1e-15);
    }

    #[test]
    fn max_spread_keeps_the_widest_spread_including_at_open() {
        let mut m = MaxSpread::default();
        assert_eq!(m.value(), None);
        m.update(&mid(0, 100.0)); // Spread 0.02.
        m.update(&quote(1, 98.0, 1.0, 102.0, 1.0)); // Spread 0.04.
        m.update(&quote(2, 99.5, 1.0, 100.5, 1.0)); // Spread 0.01.
        assert_eq!(m.value(), Some(0.04));
        m.reset();
        assert_eq!(
            m.value(),
            Some(0.01),
            "a candle without updates opens at the spread in effect, not the previous max"
        );
        m.update(&quote(3, 98.0, 1.0, 102.0, 1.0)); // Spread 0.04 again, wider than the opening 0.01.
        assert_eq!(m.value(), Some(0.04));
        // A crossed quote is skipped.
        m.update(&quote(4, 102.0, 1.0, 98.0, 1.0));
        assert_eq!(m.value(), Some(0.04));
    }

    #[test]
    fn volume_location_shares_classify_trades_against_the_prevailing_quote() {
        type Event = MarketEvent<Trade, Quote>;
        let trade = |price: f64, size: f64| {
            Event::Trade(Trade {
                timestamp: 1,
                price,
                size,
            })
        };
        let (mut at_touch, mut inside, mut through) = (
            VolumeAtTouch::default(),
            VolumeInsideSpread::default(),
            VolumeThroughTouch::default(),
        );
        assert_eq!(at_touch.value(), None, "no prevailing quote yet");

        // Prevailing quote: bid 100, ask 102.
        let q = Event::Quote(quote(0, 100.0, 1.0, 102.0, 1.0));
        at_touch.update(&q);
        inside.update(&q);
        through.update(&q);

        let events = [
            trade(102.0, 102.0),  // Buy at the ask (base size 1): at touch.
            trade(101.0, 202.0),  // Buy at 101, inside (base size 2).
            trade(103.0, 103.0),  // Buy above the ask (base size 1): through.
            trade(100.0, 100.0),  // Buy at the bid (base size 1): the wrong side, counts nowhere.
            trade(100.0, -100.0), // Sell at the bid (base size 1): at touch.
            trade(100.0, 0.0),    // Size zero, skipped entirely.
        ];
        for e in &events {
            at_touch.update(e);
            inside.update(e);
            through.update(e);
        }

        // 1 (buy) + 2 + 1 + 1 (wrong side) + 1 (sell) = 6 base volume had a prevailing quote.
        assert_eq!(at_touch.value(), Some(2.0 / 6.0));
        assert_eq!(inside.value(), Some(2.0 / 6.0));
        assert_eq!(through.value(), Some(1.0 / 6.0));
        // The wrong-side trade is in none of the three: the shares sum to less than 1.
        let sum = at_touch.value().expect("trades")
            + inside.value().expect("trades")
            + through.value().expect("trades");
        assert!(sum < 1.0, "{sum}");
    }

    #[test]
    fn volume_location_shares_treat_near_equal_decimal_prices_as_touching() {
        type Event = MarketEvent<Trade, Quote>;
        let mut at_touch = VolumeAtTouch::default();
        at_touch.update(&Event::Quote(quote(0, 100.0, 1.0, 102.0, 1.0)));
        // 102.0 parsed from two different decimal strings can differ in the last bit.
        let ask = "102.0".parse::<f64>().expect("valid float");
        let noisy_ask = ask * (1.0 + 1e-12);
        assert_ne!(ask, noisy_ask, "the float actually differs");
        at_touch.update(&Event::Trade(Trade {
            timestamp: 1,
            price: noisy_ask,
            size: noisy_ask,
        }));
        assert_eq!(
            at_touch.value(),
            Some(1.0),
            "within tolerance of the ask, counted at the touch"
        );
    }

    #[test]
    fn volume_location_shares_ignore_invalid_quotes_and_reset_the_counts_only() {
        type Event = MarketEvent<Trade, Quote>;
        let mut at_touch = VolumeAtTouch::default();
        at_touch.update(&Event::Quote(quote(0, 100.0, 1.0, 102.0, 1.0)));
        // An invalid (crossed) quote does not replace the prevailing one.
        at_touch.update(&Event::Quote(quote(1, 102.0, 1.0, 100.0, 1.0)));
        at_touch.update(&Event::Trade(Trade {
            timestamp: 2,
            price: 102.0,
            size: 102.0,
        }));
        assert_eq!(at_touch.value(), Some(1.0));

        // The counts reset, but the prevailing quote is kept, like EffectiveSpread's mid.
        at_touch.reset();
        assert_eq!(at_touch.value(), None);
        at_touch.update(&Event::Trade(Trade {
            timestamp: 3,
            price: 102.0,
            size: 102.0,
        }));
        assert_eq!(at_touch.value(), Some(1.0));
    }
}
