//! Components of the times between consecutive trades (the durations), in the units of the
//! event timestamps ([`Timestamped::TIMESTAMP_RESOLUTION`](crate::Timestamped)).
//!
//! Trades arrive in bursts: a duration predicts the next one, as the autoregressive
//! conditional duration (ACD) model of Engle and Russell (1998) describes, and short durations
//! go with information arriving, wide spreads and high volatility.
//!
//! Every trade counts, also one without size, because these components follow the arrival
//! of the trade events and not their side. Trades with the same timestamp, e.g. the fills of
//! one taker order, are a duration of 0. Trades must arrive in time order.

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, Timestamped};

/// The duration from the previous trade to `timestamp`, which then becomes the previous trade.
/// `None` for the first trade.
#[inline(always)]
fn duration_since(last: &mut Option<i64>, timestamp: i64) -> Option<i64> {
    let duration = last.replace(timestamp).map(|last| timestamp - last);
    if let Some(duration) = duration {
        assert!(duration >= 0, "trades must arrive in time order");
    }
    duration
}

/// The longest time between consecutive trades of the candle, including the time from the
/// last trade of the previous candle to the first trade of the candle,
/// in the units of the event timestamps.
/// `None` if no duration ends in the candle: without trades, or for the first trade ever.
/// Trades with the same timestamp are a duration of 0, so the value is then `Some(0)`.
///
/// A long gap is a pause of the market, e.g. before news or in a thin market, which an
/// average duration hides. Divide by the duration of the candle
/// ([`Bar::duration`](crate::Bar::duration)) or the mean duration for a scale-free value.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxGap {
    max: Option<i64>,
    last: Option<i64>,
}

impl CandleComponent for MaxGap {
    type Value = Option<i64>;

    #[inline(always)]
    fn value(&self) -> Option<i64> {
        self.max
    }

    /// Keeps the time of the last trade, for the first gap of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.max = None;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for MaxGap {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        if let Some(gap) = duration_since(&mut self.last, trade.timestamp()) {
            self.max = Some(self.max.map_or(gap, |max| max.max(gap)));
        }
        debug_assert!(self.max.is_none_or(|max| max >= 0));
    }
}

/// The coefficient of variation of the durations between consecutive trades within the
/// candle: their population standard deviation divided by their mean.
/// `None` with fewer than two durations, or if all durations are 0.
///
/// A measure of burstiness that is scale-free in time (Engle and Russell 1998):
/// about 1 for trades that arrive at random (a Poisson process, exponential durations),
/// above 1 for clustered, bursty trading, e.g. a large order or an algorithm reacting to news,
/// and below 1 for regular trading, e.g. a scheduled execution.
/// Only durations between two trades of the candle count, so that the time before the first
/// trade, which depends on where the candle starts, does not.
/// Trades with the same timestamp are durations of 0 and count: simultaneous fills are the
/// burstiest arrivals.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InterArrivalCv {
    count: u64,
    mean: f64,
    /// The sum of the squared deviations from the mean (Welford's online algorithm).
    m2: f64,
    last: Option<i64>,
}

impl CandleComponent for InterArrivalCv {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if self.count < 2 || self.mean <= 0.0 {
            return None;
        }
        let std_dev = (self.m2.max(0.0) / self.count as f64).sqrt();
        let cv = std_dev / self.mean;
        debug_assert!(cv.is_finite());
        debug_assert!(cv >= 0.0);
        Some(cv)
    }

    /// Forgets the last trade too: the durations of a candle start at its first trade.
    #[inline(always)]
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for InterArrivalCv {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let Some(duration) = duration_since(&mut self.last, trade.timestamp()) else {
            return;
        };
        let duration = duration as f64;
        self.count += 1;
        let delta = duration - self.mean;
        self.mean += delta / self.count as f64;
        self.m2 += delta * (duration - self.mean);
        debug_assert!(self.mean >= 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{trade, MicroTrade},
        Aggregator, Candle, Gaps, GenericAggregator, MarketEvent, MillisecondPeriod, Quote,
        TimeRule, Trade,
    };

    fn at(timestamps: &[i64]) -> Vec<Trade> {
        timestamps.iter().map(|&ts| trade(ts, 100.0, 1.0)).collect()
    }

    fn update_all<C: CandleComponentUpdate<E>, E>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    #[test]
    fn the_max_gap_includes_the_gap_into_the_candle() {
        let mut m = MaxGap::default();
        assert_eq!(m.value(), None);
        update_all(&mut m, &at(&[0]));
        assert_eq!(m.value(), None, "the first trade has no gap");
        update_all(&mut m, &at(&[10, 15, 45]));
        assert_eq!(m.value(), Some(30));
        m.reset();
        assert_eq!(m.value(), None, "an empty candle");
        update_all(&mut m, &at(&[100, 101]));
        assert_eq!(m.value(), Some(55), "from the last trade at 45");
        m.reset();
        // A trade without size counts, and the same timestamp is a gap of 0.
        update_all(&mut m, &[trade(101, 100.0, 0.0)]);
        assert_eq!(m.value(), Some(0));

        let mut micros = MaxGap::default();
        update_all(
            &mut micros,
            &[
                MicroTrade(trade(0, 1.0, 1.0)),
                MicroTrade(trade(2_500, 1.0, 1.0)),
            ],
        );
        assert_eq!(
            micros.value(),
            Some(2_500),
            "in the units of the timestamps"
        );
    }

    #[test]
    #[should_panic(expected = "time order")]
    fn out_of_order_trades_panic() {
        update_all(&mut MaxGap::default(), &at(&[10, 5]));
    }

    #[test]
    fn the_cv_of_the_durations_within_the_candle() {
        let cv = |timestamps: &[i64]| {
            let mut m = InterArrivalCv::default();
            update_all(&mut m, &at(timestamps));
            m.value()
        };
        assert_eq!(cv(&[]), None);
        assert_eq!(cv(&[0, 10]), None, "one duration");
        assert_eq!(cv(&[0, 10, 20, 30]), Some(0.0), "regular");
        assert_eq!(cv(&[5, 5, 5]), None, "all durations are 0");
        // Durations 10, 5 and 30: mean 15, squared deviations 25 + 100 + 225.
        let expected = (350.0_f64 / 3.0).sqrt() / 15.0;
        assert!((cv(&[0, 10, 15, 45]).expect("durations") - expected).abs() < 1e-12);
        // Durations 0, 0 and 30: mean 10, squared deviations 100 + 100 + 400.
        let bursty = cv(&[0, 0, 0, 30]).expect("durations");
        assert!((bursty - 2.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn the_cv_ignores_the_gap_into_the_candle() {
        let mut m = InterArrivalCv::default();
        update_all(&mut m, &at(&[0, 10, 20]));
        assert_eq!(m.value(), Some(0.0));
        m.reset();
        assert_eq!(m.value(), None, "an empty candle");
        update_all(&mut m, &at(&[1_000, 1_010, 1_020]));
        assert_eq!(m.value(), Some(0.0), "the gap of 980 does not count");
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct ArrivalCandle {
        max_gap: MaxGap,
        inter_arrival_cv: InterArrivalCv,
    }

    #[test]
    fn quotes_are_ignored_and_empty_candles_have_no_durations() {
        type Event = MarketEvent<Trade, Quote>;
        let rule = TimeRule::new(MillisecondPeriod::from_non_zero(100));
        let mut a = GenericAggregator::<ArrivalCandle, _, Event>::new(rule, false)
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
        let mut events = vec![quote(5)];
        for t in at(&[10, 20, 25, 55, 350, 460]) {
            events.push(Event::Trade(t));
            events.push(quote(t.timestamp + 1));
        }
        let mut bars = Vec::new();
        for e in &events {
            a.update(e, &mut bars);
        }
        assert_eq!(bars.len(), 4);
        let first = bars[0].candle();
        assert_eq!(first.max_gap(), Some(30));
        // Durations 10, 5 and 30, as above.
        let expected = (350.0_f64 / 3.0).sqrt() / 15.0;
        assert!((first.inter_arrival_cv().expect("durations") - expected).abs() < 1e-12);
        for flat in &bars[1..3] {
            assert_eq!(flat.candle().max_gap(), None);
            assert_eq!(flat.candle().inter_arrival_cv(), None);
        }
        assert_eq!(
            bars[3].candle().max_gap(),
            Some(295),
            "from the trade at 55"
        );
        assert_eq!(bars[3].candle().inter_arrival_cv(), None, "one trade");
    }
}
