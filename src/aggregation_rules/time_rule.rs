use crate::{
    aggregation_rules::clock::{Clock, Origin},
    AggregationRule, MillisecondPeriod, Timestamped,
};

/// The classic time based aggregation rule, creating a new candle every period,
/// with the periods counted from the first event.
///
/// The candles depend on where the event stream starts, so a replay that starts elsewhere
/// produces other candles. Prefer [`AlignedTimeRule`](crate::AlignedTimeRule),
/// whose periods are aligned to the epoch.
///
/// Each candle covers the half-open interval `[start, start + period)` and closes at its end,
/// see [`AggregationRule::deadline`].
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimeRule {
    clock: Clock,
}

impl TimeRule {
    /// Create a new instance of the time rule.
    ///
    /// # Arguments:
    /// `period`: How long a candle lasts.
    /// The unit of the event timestamps is [`Timestamped::TIMESTAMP_RESOLUTION`].
    pub fn new(period: MillisecondPeriod) -> Self {
        Self {
            clock: Clock::new(period, Origin::FirstEvent),
        }
    }
}

impl<E: Timestamped> AggregationRule<E> for TimeRule {
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
    use std::marker::PhantomData;

    use trade_aggregation_derive::Candle;

    use super::*;
    use crate::{
        aggregate_all_trades,
        aggregation_rules::test_utils::{closed_periods, trade, MicroTrade, NanoTrade},
        candle_components::{CandleComponent, CandleComponentUpdate, Close},
        load_trades_from_csv,
        plot::{plot_ohlc_candles, OhlcCandle},
        Bar, BarKind, Gaps, GenericAggregator, ModularCandle, Trade, H1, M15, M5,
    };

    /// The close of microsecond trades.
    #[derive(Debug, Default, Clone, Candle)]
    struct MicroCandle {
        close: Close,
        input: PhantomData<MicroTrade>,
    }

    /// The close of nanosecond trades.
    #[derive(Debug, Default, Clone, Candle)]
    struct NanoCandle {
        close: Close,
        input: PhantomData<NanoTrade>,
    }

    #[test]
    fn time_candles_plot() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator =
            GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(M15), false);
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(Bar::into_candle)
            .collect();
        println!("got {} candles", candles.len());

        plot_ohlc_candles(&candles, "img/time_candles_plot.png", (2560, 1440)).unwrap();
    }

    #[test]
    fn time_rule_counts_periods_from_the_first_trade() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();
        let timestamps: Vec<i64> = trades.iter().map(|t| t.timestamp).collect();
        let first = timestamps[0];
        let last = *timestamps.last().unwrap();
        for period in [M5, M15, H1] {
            let units = period.get() as i64;
            let mut aggregator =
                GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(period), false);
            let bars = aggregate_all_trades(&trades, &mut aggregator);
            assert_eq!(bars.len(), closed_periods(&timestamps, first, units));
            assert!(bars.iter().all(|b| b.kind() == BarKind::Deadline));
            assert!(bars.iter().all(|b| (b.close_ts() - first) % units == 0));

            // With an empty candle for each period without trades, every period closes.
            let mut aggregator =
                GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(period), false)
                    .with_gaps(Gaps::Flat {
                        max_candles: u64::MAX,
                    });
            let bars = aggregate_all_trades(&trades, &mut aggregator);
            assert_eq!(bars.len() as i64, (last - first) / units);
        }
    }

    #[test]
    fn time_rule_follows_the_timestamp_resolution_of_the_event_type() {
        let trades_ms = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();
        let scaled = |factor: i64| -> Vec<Trade> {
            trades_ms
                .iter()
                .map(|t| Trade {
                    timestamp: t.timestamp * factor,
                    ..*t
                })
                .collect()
        };
        let micros: Vec<MicroTrade> = scaled(1_000).into_iter().map(MicroTrade).collect();
        let nanos: Vec<NanoTrade> = scaled(1_000_000).into_iter().map(NanoTrade).collect();

        let ms = aggregate_all_trades(
            &trades_ms,
            &mut GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(M15), false),
        );
        let us = aggregate_all_trades(
            &micros,
            &mut GenericAggregator::<MicroCandle, TimeRule, MicroTrade>::new(
                TimeRule::new(M15),
                false,
            ),
        );
        let ns = aggregate_all_trades(
            &nanos,
            &mut GenericAggregator::<NanoCandle, TimeRule, NanoTrade>::new(
                TimeRule::new(M15),
                false,
            ),
        );
        assert_eq!(ms.len(), us.len());
        assert_eq!(ms.len(), ns.len());
        for ((a, b), c) in ms.iter().zip(&us).zip(&ns) {
            assert_eq!(a.close_ts() * 1_000, b.close_ts());
            assert_eq!(a.close_ts() * 1_000_000, c.close_ts());
            assert_eq!(a.candle().close(), c.candle().close());
        }
    }

    #[test]
    fn a_gap_closes_one_candle_not_one_per_later_trade() {
        // 13.1 advanced by one period per trade after a gap,
        // so each of the trades at 1000.. closed a candle of its own.
        let trades: Vec<Trade> = [0, 50, 1_000, 1_001, 1_002, 1_003]
            .iter()
            .map(|ts| trade(*ts, 100.0, 1.0))
            .collect();
        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(crate::MillisecondPeriod::from_non_zero(100)),
            false,
        );
        let bars = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(bars.len(), 1);
        assert_eq!((bars[0].open_ts(), bars[0].close_ts()), (0, 100));
        assert_eq!(bars[0].num_events(), 2);
        let unfinished = aggregator.provisional(1_050).expect("a candle is open");
        assert_eq!(unfinished.num_events(), 4);
    }

    #[test]
    fn used_alone_the_rule_triggers_once_per_gap() {
        let mut rule = TimeRule::new(crate::MillisecondPeriod::from_non_zero(100));
        let triggers: Vec<bool> = [0, 50, 1_000, 1_001, 1_099, 1_100]
            .iter()
            .map(|ts| AggregationRule::<Trade>::should_trigger(&mut rule, &trade(*ts, 1.0, 1.0)))
            .collect();
        assert_eq!(triggers, vec![false, false, true, false, false, true]);
    }
}
