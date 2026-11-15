//! Components on the ids of the trades ([`TradeId`]).
//!
//! A live feed loses trades (a WebSocket reconnect, a producer restart), and a backfill can have
//! holes that no public source fills. The ids show both: they increase by one per trade.

use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TradeId};

/// The number of trade ids that are missing before the trades of the candle.
///
/// The gap before a trade is the number of ids between it and the previous trade, also when
/// that trade is in the previous candle: a gap counts in the candle of the trade after it.
/// A candle with missing trades has wrong sums and counts, so mask it downstream.
///
/// An id at or below the previous one restarts the count without a gap: some exchanges
/// restart the ids when they list a symbol again (Binance, TLMUSDT on 2023-03-30).
/// Remove duplicates before the aggregator.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MissingTrades {
    /// The id of the previous trade, across candles.
    last_id: Option<u64>,
    missing: u64,
}

impl CandleComponent for MissingTrades {
    type Value = u64;

    #[inline(always)]
    fn value(&self) -> u64 {
        self.missing
    }

    /// Keeps the id of the last trade, for the gap before the first trade of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.missing = 0;
    }
}

impl<E> CandleComponentUpdate<E> for MissingTrades
where
    E: AsTrade,
    E::Trade: TradeId,
{
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let id = trade.trade_id();
        if let Some(last) = self.last_id {
            if id > last {
                self.missing += id - last - 1;
            }
        }
        self.last_id = Some(id);
    }
}

/// The id of the first trade of the candle, `None` without trades.
///
/// With [`LastTradeId`] it finds the candles that contain a range of ids, e.g. to recompute
/// them after the missing trades were repaired.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FirstTradeId {
    first: Option<u64>,
}

impl CandleComponent for FirstTradeId {
    type Value = Option<u64>;

    #[inline(always)]
    fn value(&self) -> Option<u64> {
        self.first
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.first = None;
    }
}

impl<E> CandleComponentUpdate<E> for FirstTradeId
where
    E: AsTrade,
    E::Trade: TradeId,
{
    #[inline(always)]
    fn update(&mut self, event: &E) {
        if let Some(trade) = event.as_trade() {
            self.first.get_or_insert(trade.trade_id());
        }
    }
}

/// The id of the last trade of the candle, `None` without trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LastTradeId {
    last: Option<u64>,
}

impl CandleComponent for LastTradeId {
    type Value = Option<u64>;

    #[inline(always)]
    fn value(&self) -> Option<u64> {
        self.last
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.last = None;
    }
}

impl<E> CandleComponentUpdate<E> for LastTradeId
where
    E: AsTrade,
    E::Trade: TradeId,
{
    #[inline(always)]
    fn update(&mut self, event: &E) {
        if let Some(trade) = event.as_trade() {
            self.last = Some(trade.trade_id());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        candle_components::NumTrades, Aggregator, Candle, GenericAggregator, MarketEvent, Quote,
        SizeUnit, TakerTrade, TickRule, TimestampResolution, Timestamped,
    };

    #[derive(Debug, Clone, Copy)]
    struct IdTrade {
        timestamp: i64,
        id: u64,
    }

    impl Timestamped for IdTrade {
        const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Millisecond;

        fn timestamp(&self) -> i64 {
            self.timestamp
        }
    }

    impl TakerTrade for IdTrade {
        const SIZE_UNIT: SizeUnit = SizeUnit::Base;

        fn price(&self) -> f64 {
            100.0
        }

        fn size(&self) -> f64 {
            1.0
        }
    }

    impl TradeId for IdTrade {
        fn trade_id(&self) -> u64 {
            self.id
        }
    }

    fn with_ids(ids: &[u64]) -> Vec<IdTrade> {
        ids.iter()
            .zip(0..)
            .map(|(&id, timestamp)| IdTrade { timestamp, id })
            .collect()
    }

    fn update_all<C: CandleComponentUpdate<E>, E>(c: &mut C, events: &[E]) {
        for e in events {
            c.update(e);
        }
    }

    #[test]
    fn missing_trades_counts_the_gaps_before_the_trades_of_the_candle() {
        let mut m = MissingTrades::default();
        update_all(&mut m, &with_ids(&[10]));
        assert_eq!(m.value(), 0, "the first trade has no gap");
        update_all(&mut m, &with_ids(&[11, 14, 15]));
        assert_eq!(m.value(), 2);
        m.reset();
        assert_eq!(m.value(), 0);
        update_all(&mut m, &with_ids(&[20]));
        assert_eq!(m.value(), 4, "the gap from the previous candle");
    }

    #[test]
    fn a_restart_of_the_ids_is_no_gap() {
        let mut m = MissingTrades::default();
        update_all(&mut m, &with_ids(&[1_000, 3, 3, 5]));
        assert_eq!(m.value(), 1);
    }

    #[test]
    fn first_and_last_trade_id_of_the_candle() {
        let (mut first, mut last) = (FirstTradeId::default(), LastTradeId::default());
        assert_eq!(first.value(), None);
        assert_eq!(last.value(), None);
        let trades = with_ids(&[4, 5, 9]);
        update_all(&mut first, &trades);
        update_all(&mut last, &trades);
        assert_eq!(first.value(), Some(4));
        assert_eq!(last.value(), Some(9));
        first.reset();
        last.reset();
        assert_eq!(first.value(), None);
        assert_eq!(last.value(), None);
    }

    #[test]
    fn quotes_have_no_id() {
        let quote = MarketEvent::<IdTrade, Quote>::Quote(Quote {
            timestamp: 1,
            bid_price: 99.0,
            bid_size: 1.0,
            ask_price: 101.0,
            ask_size: 1.0,
        });
        let trade = |id| MarketEvent::<IdTrade, Quote>::Trade(IdTrade { timestamp: 2, id });
        let events = [trade(7), quote, trade(9), quote];
        let (mut m, mut first, mut last) = (
            MissingTrades::default(),
            FirstTradeId::default(),
            LastTradeId::default(),
        );
        update_all(&mut m, &events);
        update_all(&mut first, &events);
        update_all(&mut last, &events);
        assert_eq!(m.value(), 1);
        assert_eq!(first.value(), Some(7));
        assert_eq!(last.value(), Some(9));
    }

    #[derive(Debug, Default, Clone, Candle)]
    struct IdCandle {
        num_trades: NumTrades<u32>,
        missing_trades: MissingTrades,
        first_trade_id: FirstTradeId,
        last_trade_id: LastTradeId,
    }

    #[test]
    fn the_candles_cover_every_id_once() {
        // Every id from 100 to 999 that is divisible by 3 is lost.
        let lost = |id: &u64| id.is_multiple_of(3);
        let ids: Vec<u64> = (100..1_000).filter(|id| !lost(id)).collect();
        let mut aggregator =
            GenericAggregator::<IdCandle, _, IdTrade>::new(TickRule::new(50), false);
        let mut bars = Vec::new();
        for t in with_ids(&ids) {
            aggregator.update(&t, &mut bars);
        }
        assert!(bars.len() > 10);
        let mut previous_last = bars[0].candle().first_trade_id().expect("a trade") - 1;
        let mut covered = 0;
        for bar in &bars {
            let c = bar.candle();
            let last = c.last_trade_id().expect("a trade");
            let lost_before = ((previous_last + 1)..=last).filter(lost).count();
            assert_eq!(c.missing_trades(), lost_before as u64);
            assert_eq!(
                u64::from(c.num_trades()) + c.missing_trades(),
                last - previous_last
            );
            covered += last - previous_last;
            previous_last = last;
        }
        let first = bars[0].candle().first_trade_id().expect("a trade");
        assert_eq!(covered, previous_last - first + 1);
    }
}
