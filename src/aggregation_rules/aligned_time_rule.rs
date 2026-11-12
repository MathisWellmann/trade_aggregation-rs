use crate::{
    aggregation_rules::clock::{Clock, Origin},
    AggregationRule, MillisecondPeriod, Timestamped,
};

/// The classic time based aggregation rule, creating a new candle every period,
/// with the periods aligned to the Unix epoch (plus an optional offset).
///
/// For example, if the first event comes at 1:32:00 with 5 minute candles,
/// the first candle covers 1:30:00 to 1:35:00 and holds 3 minutes of events.
/// Because the periods do not depend on the first event, a replay that starts anywhere
/// produces the same candles from the second candle on.
///
/// Each candle covers the half-open interval `[start, start + period)` and closes at its end,
/// see [`AggregationRule::deadline`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlignedTimeRule {
    clock: Clock,
}

impl AlignedTimeRule {
    /// Create a new instance of the aligned time rule.
    ///
    /// # Arguments:
    /// `period`: How long a candle lasts, e.g. [`M1`](crate::M1).
    /// The unit of the event timestamps is [`Timestamped::TIMESTAMP_RESOLUTION`].
    pub fn new(period: MillisecondPeriod) -> Self {
        Self::with_offset_ms(period, 0)
    }

    /// Create a new instance whose periods start `offset_ms` milliseconds after the epoch
    /// (and every period from there), e.g. weekly candles that start on a Monday,
    /// as the epoch was a Thursday.
    pub fn with_offset_ms(period: MillisecondPeriod, offset_ms: u64) -> Self {
        Self {
            clock: Clock::new(period, Origin::Epoch { offset_ms }),
        }
    }
}

impl<E: Timestamped> AggregationRule<E> for AlignedTimeRule {
    /// Starts the clock at the first event.
    /// An event past the current period triggers and moves the clock to its own period,
    /// so a gap produces one candle, not one candle per later event.
    fn should_trigger(&mut self, event: &E) -> bool {
        self.clock
            .observe(event.timestamp(), E::TIMESTAMP_RESOLUTION)
    }

    fn deadline(&self) -> Option<i64> {
        self.clock.deadline(E::TIMESTAMP_RESOLUTION)
    }

    fn on_deadline(&mut self) {
        self.clock.advance(E::TIMESTAMP_RESOLUTION);
    }

    fn skip_to(&mut self, timestamp: i64) {
        self.clock.skip_to(timestamp, E::TIMESTAMP_RESOLUTION);
    }
}

#[cfg(test)]
mod tests {
    use trade_aggregation_derive::Candle;

    use super::*;
    use crate::{
        aggregate_all_trades,
        aggregation_rules::test_utils::{closed_periods, trade, MicroTrade},
        candle_components::{Close, High, Low, NumTrades, Open, Volume},
        load_trades_from_csv,
        plot::OhlcCandle,
        Aggregator, BarKind, Gaps, GenericAggregator, Trade, M1, M15,
    };

    /// The candle of microsecond trades.
    #[derive(Default, Debug, Clone, Candle)]
    struct MicroCandle {
        num_trades: NumTrades<u32>,
        volume: Volume,
    }

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
        high: High,
        low: Low,
        close: Close,
        num_trades: NumTrades<u32>,
        volume: Volume,
    }

    #[test]
    fn aligned_time_rule() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<MyCandle, AlignedTimeRule, Trade>::new(
            AlignedTimeRule::new(M15),
            false,
        );
        let bars = aggregate_all_trades(&trades, &mut aggregator);
        let timestamps: Vec<i64> = trades.iter().map(|t| t.timestamp).collect();
        let period = M15.get() as i64;
        assert_eq!(bars.len(), closed_periods(&timestamps, 0, period));
        assert!(bars.iter().all(|b| b.close_ts() % period == 0));
        let candles: Vec<_> = bars.into_iter().map(crate::Bar::into_candle).collect();

        // make sure that the aggregator starts a new candle with the "trigger tick",
        // and includes that information of the trade that triggered the new candle as well
        let c = &candles[0];
        assert_eq!(c.open(), 13873.0);
        assert_eq!(c.close(), 13769.0);
        let c = &candles[1];
        assert_eq!(c.open(), 13768.5);
        assert_eq!(c.close(), 13721.5);
    }

    #[test]
    fn aligned_time_rule_volume() {
        // The Bitstamp timestamps are in microseconds.
        let trades: Vec<MicroTrade> = load_trades_from_csv("data/Bitstamp_BTCEUR_1M.csv")
            .unwrap()
            .into_iter()
            .map(MicroTrade)
            .collect();

        let mut aggregator = GenericAggregator::<MicroCandle, AlignedTimeRule, MicroTrade>::new(
            AlignedTimeRule::new(M1),
            false,
        );
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();

        let c = &candles[0];
        assert_eq!(c.num_trades(), 10);
        assert_eq!(c.volume(), 0.27458132);
    }

    #[test]
    fn aligned_time_rule_trigger_on_0() {
        let trades: [Trade; 5] = [
            Trade {
                timestamp: 1712656800000,
                price: 100.0,
                size: 10.0,
            },
            Trade {
                timestamp: 1712656815000,
                price: 101.0,
                size: -10.0,
            },
            Trade {
                timestamp: 1712656860000,
                price: 100.5,
                size: -10.0,
            },
            Trade {
                timestamp: 1712656860001,
                price: 102.0,
                size: -10.0,
            },
            Trade {
                timestamp: 1712656935000,
                price: 105.0,
                size: -10.0,
            },
        ];

        let mut aggregator = GenericAggregator::<OhlcCandle, AlignedTimeRule, Trade>::new(
            AlignedTimeRule::new(M1),
            false,
        );
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].open(), 100.00);
        assert_eq!(candles[0].close(), 101.00);
        assert_eq!(candles[1].open(), 100.5);
        assert_eq!(candles[1].close(), 102.00);
    }

    #[test]
    fn aligned_time_rule_candle_with_one_trade() {
        let trades: [Trade; 4] = [
            Trade {
                timestamp: 1712656800000,
                price: 100.0,
                size: 10.0,
            },
            Trade {
                timestamp: 1712656815000,
                price: 101.0,
                size: -10.0,
            },
            Trade {
                timestamp: 1712656861000,
                price: 100.5,
                size: -10.0,
            },
            Trade {
                timestamp: 1712657930000,
                price: 102.0,
                size: -10.0,
            },
        ];

        let mut aggregator = GenericAggregator::<OhlcCandle, AlignedTimeRule, Trade>::new(
            AlignedTimeRule::new(M1),
            false,
        );
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].open(), 100.0);
        assert_eq!(candles[0].close(), 101.0);
        assert_eq!(candles[1].open(), 100.5);
        assert_eq!(candles[1].close(), 100.5);
    }

    /// Trades at the given milliseconds and prices, each of size 1.
    fn trades_at(points: &[(i64, f64)]) -> Vec<Trade> {
        points.iter().map(|(ts, p)| trade(*ts, *p, 1.0)).collect()
    }

    fn period_100ms() -> AlignedTimeRule {
        AlignedTimeRule::new(crate::MillisecondPeriod::from_non_zero(100))
    }

    #[test]
    fn a_candle_closes_at_the_period_end_on_a_watermark() {
        let mut a = GenericAggregator::<MyCandle, _, Trade>::new(period_100ms(), false);
        let mut bars = Vec::new();
        for t in &trades_at(&[(10, 1.0), (50, 2.0)]) {
            a.update(t, &mut bars);
        }
        a.advance_to(99, &mut bars);
        assert!(bars.is_empty());
        a.advance_to(100, &mut bars);
        assert_eq!(bars.len(), 1);
        let bar = &bars[0];
        assert_eq!(bar.kind(), BarKind::Deadline);
        assert_eq!((bar.open_ts(), bar.close_ts()), (10, 100));
        assert_eq!(bar.num_events(), 2);
        assert_eq!(bar.candle().close(), 2.0);
        // Without events no more candles close.
        a.advance_to(1_000, &mut bars);
        assert_eq!(bars.len(), 1);
    }

    #[test]
    fn an_event_at_the_period_end_opens_the_next_candle_for_every_trigger_trade() {
        for trigger_trade in [
            crate::TriggerTrade::NextCandle,
            crate::TriggerTrade::ClosingCandle,
            crate::TriggerTrade::Both,
        ] {
            let mut a = GenericAggregator::<MyCandle, _, Trade>::with_trigger_trade(
                period_100ms(),
                trigger_trade,
            );
            let mut bars = Vec::new();
            for t in &trades_at(&[(10, 1.0), (100, 2.0)]) {
                a.update(t, &mut bars);
            }
            assert_eq!(bars.len(), 1);
            assert_eq!(bars[0].candle().close(), 1.0, "{trigger_trade:?}");
            assert_eq!(bars[0].candle().num_trades(), 1, "{trigger_trade:?}");
            assert_eq!(a.unfinished_candle().open(), 2.0, "{trigger_trade:?}");
        }
    }

    #[test]
    fn flat_candles_fill_a_gap_up_to_the_limit() {
        let mut a = GenericAggregator::<MyCandle, _, Trade>::new(period_100ms(), false)
            .with_gaps(Gaps::Flat { max_candles: 3 });
        let mut bars = Vec::new();
        for t in &trades_at(&[(10, 1.0), (50, 3.0), (90, 2.0), (1_010, 5.0), (1_150, 6.0)]) {
            a.update(t, &mut bars);
        }
        let stamps: Vec<(i64, i64, u64)> = bars
            .iter()
            .map(|b| (b.open_ts(), b.close_ts(), b.num_events()))
            .collect();
        assert_eq!(
            stamps,
            vec![
                (10, 100, 3),
                (100, 200, 0),
                (200, 300, 0),
                (300, 400, 0),
                // The rest of the gap is skipped, so the candle of 1000 opened at 400.
                (400, 1_100, 1),
            ]
        );
        for flat in &bars[1..4] {
            let c = flat.candle();
            assert_eq!(c.open(), 2.0);
            assert_eq!(c.high(), 2.0);
            assert_eq!(c.low(), 2.0);
            assert_eq!(c.close(), 2.0);
            assert_eq!(c.volume(), 0.0);
        }
        let after_gap = bars[4].candle();
        assert_eq!(after_gap.open(), 5.0);
        assert_eq!(after_gap.high(), 5.0);
        assert_eq!(after_gap.low(), 5.0);
    }

    #[test]
    fn an_offset_moves_the_period_boundaries() {
        let rule =
            AlignedTimeRule::with_offset_ms(crate::MillisecondPeriod::from_non_zero(100), 30);
        let mut a = GenericAggregator::<MyCandle, _, Trade>::new(rule, false);
        let mut bars = Vec::new();
        for t in &trades_at(&[(10, 1.0), (40, 2.0)]) {
            a.update(t, &mut bars);
        }
        assert_eq!(bars.len(), 1);
        assert_eq!((bars[0].open_ts(), bars[0].close_ts()), (10, 30));
    }

    #[test]
    fn negative_timestamps_align_to_the_epoch() {
        let mut a = GenericAggregator::<MyCandle, _, Trade>::new(period_100ms(), false);
        let mut bars = Vec::new();
        for t in &trades_at(&[(-150, 1.0), (-10, 2.0), (0, 3.0)]) {
            a.update(t, &mut bars);
        }
        let closes: Vec<i64> = bars.iter().map(|b| b.close_ts()).collect();
        assert_eq!(closes, vec![-100, 0]);
    }
}
