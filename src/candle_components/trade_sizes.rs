//! Components of the distribution of the trade sizes, in the base currency.
//!
//! [`SizeHerfindahl`] and [`LargestTradeShare`] measure how concentrated the volume of a
//! candle is in a few trades, scale-free. [`SmallTradeFlow`], [`MediumTradeFlow`] and
//! [`LargeTradeFlow`] split the net flow by the size of the trades relative to the recent
//! average size, because traders of different sizes carry different information.

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// The Herfindahl-Hirschman index of the base sizes `v_i` of the trades:
/// `sum v_i^2 / (sum v_i)^2`, in `[1 / n, 1]` for `n` trades. `None` without volume.
///
/// Its inverse is the effective number of trades: the number of equal trades that would
/// give the same concentration, so `NumTrades * SizeHerfindahl` is 1 for equal trades and
/// grows when a few large trades carry the volume, e.g. of an informed or a forced trader.
/// Scale-free; the index of Herfindahl (1950) and Hirschman (1945) for market concentration.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SizeHerfindahl {
    volume: f64,
    squares: f64,
}

impl CandleComponent for SizeHerfindahl {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if self.volume <= 0.0 {
            return None;
        }
        let index = self.squares / (self.volume * self.volume);
        debug_assert!(index > 0.0, "{index}");
        debug_assert!(index <= 1.0 + 1e-12, "{index}");
        Some(index)
    }

    #[inline(always)]
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for SizeHerfindahl {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let size = trade.base_size().abs();
        self.volume += size;
        self.squares += size * size;
    }
}

/// The share of the largest trade in the base volume of the candle, in `(0, 1]`.
/// `None` without volume.
///
/// The scale-free companion of [`LargestTrade`](super::LargestTrade):
/// a value near 1 means that one trade, e.g. a block or a liquidation, made the candle.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LargestTradeShare {
    volume: f64,
    largest: f64,
}

impl CandleComponent for LargestTradeShare {
    type Value = Option<f64>;

    #[inline(always)]
    fn value(&self) -> Option<f64> {
        if self.volume <= 0.0 {
            return None;
        }
        let share = self.largest / self.volume;
        debug_assert!(share > 0.0, "{share}");
        debug_assert!(share <= 1.0 + 1e-12, "{share}");
        Some(share)
    }

    #[inline(always)]
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for LargestTradeShare {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let size = trade.base_size().abs();
        self.volume += size;
        self.largest = self.largest.max(size);
    }
}

/// The class of a trade by its size relative to the recent average size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SizeClass {
    Small,
    Medium,
    Large,
}

/// The exponentially weighted moving average of the base trade size, which classifies
/// each trade by the average before it, so that a trade does not dilute its own class.
///
/// The average spans candles, and the first trade seeds it with its own size.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct SizeClassifier<const SPAN: u32, const SMALL_PERCENT: u32, const LARGE_PERCENT: u32> {
    average: Option<f64>,
}

impl<const SPAN: u32, const SMALL_PERCENT: u32, const LARGE_PERCENT: u32>
    SizeClassifier<SPAN, SMALL_PERCENT, LARGE_PERCENT>
{
    /// Rejects invalid parameters when the component is compiled for them.
    const VALID: () = {
        assert!(SPAN >= 1, "SPAN must be at least 1");
        assert!(
            SMALL_PERCENT <= LARGE_PERCENT,
            "SMALL_PERCENT must not exceed LARGE_PERCENT"
        );
    };

    /// The weight of the newest trade, `2 / (SPAN + 1)`, in `(0, 1]`.
    const ALPHA: f64 = 2.0 / (SPAN as f64 + 1.0);

    /// The class and the signed base size of `trade`, after which its size joins the average.
    /// `None` for a trade without size, which has no side and would drag the average to 0.
    #[inline(always)]
    fn observe<T: TakerTrade>(&mut self, trade: &T) -> Option<(SizeClass, f64)> {
        let () = Self::VALID;
        let signed = trade.base_size();
        if signed == 0.0 {
            return None;
        }
        let size = signed.abs();
        let average = *self.average.get_or_insert(size);
        debug_assert!(average > 0.0, "the average of positive sizes");
        let class = if size * 100.0 < f64::from(SMALL_PERCENT) * average {
            SizeClass::Small
        } else if size * 100.0 >= f64::from(LARGE_PERCENT) * average {
            SizeClass::Large
        } else {
            SizeClass::Medium
        };
        self.average = Some(average + Self::ALPHA * (size - average));
        Some((class, signed))
    }
}

/// Defines a component of the signed base volume of the trades of one [`SizeClass`].
macro_rules! size_class_flow {
    ($(#[$doc:meta])* $name:ident, $class:expr) => {
        $(#[$doc])*
        ///
        /// A trade is small if its base size is below `SMALL_PERCENT` % of the exponentially
        /// weighted moving average of the base trade size with `alpha = 2 / (SPAN + 1)`,
        /// large if it is at least `LARGE_PERCENT` % of it, and medium otherwise.
        /// The trade is classified by the average before it, then joins the average.
        /// The average spans candles, and the first trade seeds it with its own size,
        /// so it is classified against itself (100 %).
        /// Trades without size are skipped. `SPAN` must be at least 1 and `SMALL_PERCENT`
        /// at most `LARGE_PERCENT`, which the compiler checks when it builds the update.
        ///
        /// Why: traders of different sizes carry different information. Informed traders
        /// hide in medium sizes ("stealth trading", Barclay and Warner 1993), retail trades
        /// small and whales or liquidations trade large. The value is the signed base volume
        /// of the class (buys minus sells, 0 without such trades); the three classes with the
        /// same parameters sum to [`NetFlow`](super::NetFlow). Divide by the
        /// [`BaseVolume`](super::BaseVolume) for a scale-free share. Additive.
        #[derive(Debug, Default, Clone)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name<
            const SPAN: u32 = 1000,
            const SMALL_PERCENT: u32 = 50,
            const LARGE_PERCENT: u32 = 500,
        > {
            flow: f64,
            sizes: SizeClassifier<SPAN, SMALL_PERCENT, LARGE_PERCENT>,
        }

        impl<const SPAN: u32, const SMALL_PERCENT: u32, const LARGE_PERCENT: u32> CandleComponent
            for $name<SPAN, SMALL_PERCENT, LARGE_PERCENT>
        {
            type Value = f64;

            #[inline(always)]
            fn value(&self) -> f64 {
                self.flow
            }

            /// Keeps the average trade size, which spans candles.
            #[inline(always)]
            fn reset(&mut self) {
                self.flow = 0.0;
            }
        }

        impl<E: AsTrade, const SPAN: u32, const SMALL_PERCENT: u32, const LARGE_PERCENT: u32>
            CandleComponentUpdate<E> for $name<SPAN, SMALL_PERCENT, LARGE_PERCENT>
        {
            #[inline(always)]
            fn update(&mut self, event: &E) {
                let Some(trade) = event.as_trade() else {
                    return;
                };
                if let Some((class, size)) = self.sizes.observe(trade) {
                    if class == $class {
                        self.flow += size;
                    }
                }
            }
        }
    };
}

size_class_flow!(
    /// The signed base volume of the small trades.
    ///
    /// ```
    /// use trade_aggregation::{candle_components::SmallTradeFlow, CandleComponent, CandleComponentUpdate, Trade};
    /// // Small below 50 % and large from 500 % of the average of about the last 100 trades.
    /// let mut flow = SmallTradeFlow::<100, 50, 500>::default();
    /// flow.update(&Trade { timestamp: 0, price: 1.0, size: 10.0 });
    /// flow.update(&Trade { timestamp: 1, price: 1.0, size: -1.0 });
    /// assert_eq!(flow.value(), -1.0);
    /// ```
    ///
    /// Invalid parameters do not compile:
    ///
    /// ```compile_fail
    /// use trade_aggregation::{candle_components::SmallTradeFlow, CandleComponent, CandleComponentUpdate, Trade};
    /// let mut flow = SmallTradeFlow::<100, 600, 500>::default();
    /// flow.update(&Trade { timestamp: 0, price: 1.0, size: 10.0 });
    /// ```
    SmallTradeFlow,
    SizeClass::Small
);

size_class_flow!(
    /// The signed base volume of the medium trades.
    MediumTradeFlow,
    SizeClass::Medium
);

size_class_flow!(
    /// The signed base volume of the large trades.
    LargeTradeFlow,
    SizeClass::Large
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trade, candle_components::NetFlow, Aggregator, Candle, Gaps,
        GenericAggregator, MarketEvent, MillisecondPeriod, Quote, SizeUnit, TimeRule,
        TimestampResolution, Timestamped, Trade,
    };

    /// A spot trade: the size is in the base currency.
    #[derive(Debug, Clone, Copy)]
    struct Spot(f64);

    impl Timestamped for Spot {
        const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Millisecond;

        fn timestamp(&self) -> i64 {
            0
        }
    }

    impl TakerTrade for Spot {
        const SIZE_UNIT: SizeUnit = SizeUnit::Base;

        fn price(&self) -> f64 {
            50.0
        }

        fn size(&self) -> f64 {
            self.0
        }
    }

    fn spots(sizes: &[f64]) -> Vec<Spot> {
        sizes.iter().map(|&s| Spot(s)).collect()
    }

    /// Inverse trades at a price of 100 with the given base sizes.
    fn inverse(sizes: &[f64]) -> Vec<Trade> {
        sizes
            .iter()
            .zip(0..)
            .map(|(&s, ts)| trade(ts, 100.0, s * 100.0))
            .collect()
    }

    fn update_all<C: CandleComponentUpdate<E>, E>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    #[test]
    fn herfindahl_and_largest_share_of_the_base_sizes() {
        let sizes = [1.0, -1.0, 0.0, 2.0];
        let (mut hhi, mut share) = (SizeHerfindahl::default(), LargestTradeShare::default());
        assert_eq!(hhi.value(), None);
        assert_eq!(share.value(), None);
        update_all(&mut hhi, &spots(&sizes));
        update_all(&mut share, &spots(&sizes));
        // (1 + 1 + 4) / 4^2, and 2 of 4.
        assert_eq!(hhi.value(), Some(0.375));
        assert_eq!(share.value(), Some(0.5));
        hhi.reset();
        share.reset();
        update_all(&mut hhi, &inverse(&sizes));
        update_all(&mut share, &inverse(&sizes));
        assert_eq!(hhi.value(), Some(0.375));
        assert_eq!(share.value(), Some(0.5));
        hhi.reset();
        share.reset();
        update_all(&mut hhi, &spots(&[0.0]));
        update_all(&mut share, &spots(&[0.0]));
        assert_eq!(hhi.value(), None, "no volume");
        assert_eq!(share.value(), None, "no volume");
    }

    /// With `SPAN = 1` the average is the size of the previous trade.
    type Small = SmallTradeFlow<1, 50, 500>;
    type Medium = MediumTradeFlow<1, 50, 500>;
    type Large = LargeTradeFlow<1, 50, 500>;

    fn flows<E>(events: &[E]) -> (f64, f64, f64)
    where
        Small: CandleComponentUpdate<E>,
        Medium: CandleComponentUpdate<E>,
        Large: CandleComponentUpdate<E>,
    {
        let (mut s, mut m, mut l) = (Small::default(), Medium::default(), Large::default());
        update_all(&mut s, events);
        update_all(&mut m, events);
        update_all(&mut l, events);
        (s.value(), m.value(), l.value())
    }

    /// Against the previous size: seed (100 %), 25 %, 600 %, 67 %, 50 % and 500 %.
    const SIZES: [f64; 6] = [2.0, -0.5, 3.0, 2.0, -1.0, 5.0];

    #[test]
    fn trades_are_classified_by_the_average_before_them() {
        // 50 % is not small and 500 % is large.
        assert_eq!(flows(&spots(&SIZES)), (-0.5, 3.0, 8.0));
        assert_eq!(flows(&inverse(&SIZES)), (-0.5, 3.0, 8.0));
        // With alpha = 1/2: 1 is 25 % of 4 and the next 4 is 160 % of 2.5,
        // but only 123 % of the average that includes it.
        let mut large = LargeTradeFlow::<3, 50, 150>::default();
        update_all(&mut large, &spots(&[4.0, 1.0, -4.0]));
        assert_eq!(large.value(), -4.0);
    }

    #[test]
    fn the_size_classes_partition_the_net_flow() {
        let sizes = [0.3, -2.0, 0.1, 7.0, -0.4, -0.05, 1.0, 0.0, 9.0, -3.0];
        let events = spots(&sizes);
        let (s, m, l) = flows(&events);
        let mut net = NetFlow::default();
        update_all(&mut net, &events);
        assert!((s + m + l - net.value()).abs() < 1e-12);
        assert!(s != 0.0);
        assert!(m != 0.0);
        assert!(l != 0.0);
    }

    #[test]
    fn the_average_spans_candles_and_skips_trades_without_size() {
        let mut small = Small::default();
        update_all(&mut small, &spots(&[2.0]));
        assert_eq!(small.value(), 0.0);
        small.reset();
        assert_eq!(small.value(), 0.0, "an empty candle");
        // A size of 0 would make the next trade infinitely large.
        update_all(&mut small, &spots(&[0.0, -0.5]));
        assert_eq!(
            small.value(),
            -0.5,
            "small against the 2 of the previous candle"
        );
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct SizeCandle {
        size_herfindahl: SizeHerfindahl,
        largest_trade_share: LargestTradeShare,
        small: Small,
        medium: Medium,
        large: Large,
    }

    #[test]
    fn quotes_are_ignored_and_empty_candles_have_no_sizes() {
        type Event = MarketEvent<Trade, Quote>;
        let rule = TimeRule::new(MillisecondPeriod::from_non_zero(100));
        let mut a = GenericAggregator::<SizeCandle, _, Event>::new(rule, false)
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
        let mut events = Vec::new();
        for (t, ts) in inverse(&SIZES).into_iter().zip([10, 20, 30, 350, 360, 460]) {
            events.push(Event::Trade(Trade { timestamp: ts, ..t }));
            events.push(quote(ts));
        }
        let mut bars = Vec::new();
        for e in &events {
            a.update(e, &mut bars);
        }
        assert_eq!(bars.len(), 4);
        let first = bars[0].candle();
        assert_eq!(first.small(), -0.5);
        assert_eq!(first.medium(), 2.0);
        assert_eq!(first.large(), 3.0);
        assert_eq!(first.largest_trade_share(), Some(3.0 / 5.5));
        for flat in &bars[1..3] {
            let c = flat.candle();
            assert_eq!(c.size_herfindahl(), None);
            assert_eq!(c.largest_trade_share(), None);
            assert_eq!((c.small(), c.medium(), c.large()), (0.0, 0.0, 0.0));
        }
        // 2 against the 3 before the gap, -1 against 2.
        assert_eq!(bars[3].candle().medium(), 1.0);
    }
}
