//! Components of the taker orders behind the trades, and of the price levels they sweep.
//! See [`TakerOrders`] for how the trades are grouped into orders.

use super::trade_signs::Side;
use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// The taker order of the last fill.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct OpenOrder {
    timestamp: i64,
    side: Side,
    first_price: f64,
    swept: bool,
}

/// What a fill adds to the statistics of its taker order.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fill {
    /// The fill starts a taker order.
    new_order: bool,
    /// The fill is the first at another price than the first fill of its order.
    new_sweep: bool,
    /// `|ln(price / first price)|` of the order.
    depth: f64,
}

/// Groups the fills into taker orders.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct TakerOrderTracker {
    order: Option<OpenOrder>,
}

impl TakerOrderTracker {
    /// The fill of `trade` in its taker order, `None` for a trade without size.
    #[inline(always)]
    fn observe<T: TakerTrade>(&mut self, trade: &T) -> Option<Fill> {
        let side = Side::of(trade)?;
        let (timestamp, price) = (trade.timestamp(), trade.price());
        assert!(price.is_finite(), "prices must be finite, got {price}");
        assert!(price > 0.0, "prices must be positive, got {price}");
        match &mut self.order {
            Some(order) if order.timestamp == timestamp && order.side == side => {
                let new_sweep = !order.swept && price != order.first_price;
                order.swept |= new_sweep;
                let depth = (price / order.first_price).ln().abs();
                debug_assert!(depth.is_finite());
                Some(Fill {
                    new_order: false,
                    new_sweep,
                    depth,
                })
            }
            _ => {
                self.order = Some(OpenOrder {
                    timestamp,
                    side,
                    first_price: price,
                    swept: false,
                });
                Some(Fill {
                    new_order: true,
                    new_sweep: false,
                    depth: 0.0,
                })
            }
        }
    }
}

/// The number of taker orders that started in the candle: runs of consecutive fills with the
/// same timestamp and side. Additive.
///
/// The number of trades counts the fills, which grow with the size of the orders and with how
/// thin the book is; the number of orders counts the decisions of the takers.
/// `NumTrades / TakerOrders` is the average number of fills per order, scale-free.
///
/// An exchange reports one trade per fill: a taker order that is larger than the queue at the
/// best price fills against several makers and, if it is large enough, at several prices.
/// The assumptions of the grouping, shared by [`Sweeps`] and [`MaxSweepDepth`]:
/// - The feed reports every fill, or merges the fills of an order at the same price;
///   a feed that merges a whole order into one trade has no sweeps.
/// - Two takers on the same side with the same timestamp count as one order,
///   so coarse timestamps (e.g. milliseconds) undercount the orders of a busy market.
/// - Other events, e.g. quotes, between the fills of an order do not end it.
/// - A trade without size has no side and is skipped: it does not end an order.
///
/// The open order spans candles: if a candle closes between two fills of an order,
/// the order counts in the candle of its first fill, and its fills in the next candle
/// continue it.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TakerOrders {
    orders: u64,
    tracker: TakerOrderTracker,
}

impl CandleComponent for TakerOrders {
    type Value = u64;

    #[inline(always)]
    fn value(&self) -> u64 {
        self.orders
    }

    /// Keeps the open order, whose fills in the next candle continue it.
    #[inline(always)]
    fn reset(&mut self) {
        self.orders = 0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for TakerOrders {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(fill) = event.as_trade().and_then(|t| self.tracker.observe(t)) else {
            return;
        };
        if fill.new_order {
            self.orders += 1;
        }
    }
}

/// The number of taker orders that swept the book: that filled at more than one price,
/// so they took all the liquidity of at least one price level. Additive.
///
/// An order counts once, in the candle of its first fill at a second price.
/// Sweeps are urgent, aggressive orders, e.g. of informed traders or liquidations, and they
/// leave the book thin; divide by [`TakerOrders`] for the share of orders that swept.
/// See [`TakerOrders`] for the assumptions.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Sweeps {
    sweeps: u64,
    tracker: TakerOrderTracker,
}

impl CandleComponent for Sweeps {
    type Value = u64;

    #[inline(always)]
    fn value(&self) -> u64 {
        self.sweeps
    }

    /// Keeps the open order, which counts once even if it sweeps further in the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sweeps = 0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for Sweeps {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(fill) = event.as_trade().and_then(|t| self.tracker.observe(t)) else {
            return;
        };
        if fill.new_sweep {
            self.sweeps += 1;
        }
    }
}

/// The deepest sweep of the candle: the largest `|ln(fill price / first fill price)|`
/// over the fills of the candle, with the first fill of each fill's taker order.
/// 0.0 if no order filled at more than one price.
///
/// A taker order walks the book away from its first price, so the farthest fill is its last
/// one; taking the maximum over the fills seen so far needs no fill after the order, which
/// may only come in the next candle. For an order that spans a candle boundary, the fills of
/// each candle count against the first fill, which may be in the previous candle.
/// A scale-free measure of how thin the book was when it mattered: the price impact of the
/// most aggressive order. See [`TakerOrders`] for the assumptions.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MaxSweepDepth {
    depth: f64,
    tracker: TakerOrderTracker,
}

impl CandleComponent for MaxSweepDepth {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.depth
    }

    /// Keeps the open order, whose fills in the next candle count against its first price.
    #[inline(always)]
    fn reset(&mut self) {
        self.depth = 0.0;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for MaxSweepDepth {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(fill) = event.as_trade().and_then(|t| self.tracker.observe(t)) else {
            return;
        };
        self.depth = self.depth.max(fill.depth);
        debug_assert!(self.depth >= 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::trade, Aggregator, Candle, Gaps, GenericAggregator,
        MarketEvent, MillisecondPeriod, ModularCandle, Quote, TimeRule, Trade,
    };

    #[derive(Debug, Default, Clone, Candle)]
    struct OrderCandle {
        taker_orders: TakerOrders,
        sweeps: Sweeps,
        max_sweep_depth: MaxSweepDepth,
    }

    impl OrderCandle {
        fn values(&self) -> (u64, u64, f64) {
            (self.taker_orders(), self.sweeps(), self.max_sweep_depth())
        }
    }

    fn update_all<E>(c: &mut OrderCandle, events: &[E])
    where
        OrderCandle: ModularCandle<E>,
    {
        for e in events {
            c.update(e);
        }
    }

    /// Fills of three orders: a buy that sweeps from 100 to 102, a sell at the same time,
    /// and a sell one millisecond later that sweeps to 98. The trade without size is skipped.
    fn fills() -> Vec<Trade> {
        vec![
            trade(1, 100.0, 1.0),
            trade(1, 100.0, 2.0),
            trade(1, 101.0, 1.0),
            trade(1, 90.0, 0.0),
            trade(1, 102.0, 1.0),
            trade(1, 99.0, -1.0),
            trade(2, 99.0, -1.0),
            trade(2, 98.0, -1.0),
        ]
    }

    #[test]
    fn fills_with_the_same_time_and_side_are_one_order() {
        let mut c = OrderCandle::default();
        assert_eq!(c.values(), (0, 0, 0.0));
        update_all(&mut c, &fills());
        assert_eq!(c.values(), (3, 2, (102.0_f64 / 100.0).ln()));
        ModularCandle::<Trade>::reset(&mut c);
        assert_eq!(c.values(), (0, 0, 0.0), "an empty candle");
        update_all(&mut c, &[trade(3, 98.0, -1.0)]);
        assert_eq!(c.values(), (1, 0, 0.0), "a new timestamp starts an order");
    }

    #[test]
    fn an_order_spanning_a_boundary_counts_once() {
        let mut c = OrderCandle::default();
        update_all(&mut c, &[trade(5, 100.0, 1.0), trade(5, 101.0, 1.0)]);
        assert_eq!(c.values(), (1, 1, (101.0_f64 / 100.0).ln()));
        ModularCandle::<Trade>::reset(&mut c);
        update_all(&mut c, &[trade(5, 102.0, 1.0)]);
        assert_eq!(
            c.values(),
            (0, 0, (102.0_f64 / 100.0).ln()),
            "the depth from the first fill in the previous candle"
        );

        let mut late = OrderCandle::default();
        update_all(&mut late, &[trade(5, 100.0, -1.0)]);
        assert_eq!(late.values(), (1, 0, 0.0));
        ModularCandle::<Trade>::reset(&mut late);
        update_all(&mut late, &[trade(5, 100.0, -1.0), trade(5, 99.0, -1.0)]);
        assert_eq!(
            late.values(),
            (0, 1, (99.0_f64 / 100.0).ln().abs()),
            "the sweep counts where the second price appears"
        );
    }

    #[test]
    fn quotes_between_fills_do_not_end_an_order() {
        type Event = MarketEvent<Trade, Quote>;
        let quote = |timestamp| {
            Event::Quote(Quote {
                timestamp,
                bid_price: 99.0,
                bid_size: 1.0,
                ask_price: 101.0,
                ask_size: 1.0,
            })
        };
        let events: Vec<Event> = fills()
            .into_iter()
            .flat_map(|t| [Event::Trade(t), quote(t.timestamp)])
            .collect();
        let mut c = OrderCandle::default();
        update_all(&mut c, &events);
        assert_eq!(c.values(), (3, 2, (102.0_f64 / 100.0).ln()));
    }

    #[test]
    fn empty_candles_have_no_orders() {
        let rule = TimeRule::new(MillisecondPeriod::from_non_zero(100));
        let mut a = GenericAggregator::<OrderCandle, _, Trade>::new(rule, false)
            .with_gaps(Gaps::Flat { max_candles: 3 });
        let mut bars = Vec::new();
        let mut trades = fills();
        trades.extend([trade(350, 100.0, 1.0), trade(460, 100.0, 1.0)]);
        for t in &trades {
            a.update(t, &mut bars);
        }
        assert_eq!(bars.len(), 4);
        assert_eq!(bars[0].candle().values(), (3, 2, (102.0_f64 / 100.0).ln()));
        for flat in &bars[1..3] {
            assert_eq!(flat.candle().values(), (0, 0, 0.0));
        }
        assert_eq!(bars[3].candle().values(), (1, 0, 0.0));
    }
}
