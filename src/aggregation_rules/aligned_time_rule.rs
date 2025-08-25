use crate::{AggregationRule, MillisecondPeriod, ModularCandle, TakerTrade, TimestampResolution};

/// The classic time based aggregation rule,
/// creating a new candle every n seconds.  The time trigger is aligned such that
/// the trigger points are starting from a time equals zero.  For example, if the first
/// tick comes in a 1:32:00 on a 5 minute candle, that first candle will only contain
/// 3 minutes of trades, representing a 1:30 start.
#[derive(Debug, Clone)]
pub struct AlignedTimeRule {
    // The timestamp this rule uses as a reference
    reference_timestamp: i64,

    // The period for the candle in seconds
    // constants can be used nicely here from constants.rs
    // e.g.: M1 -> 1 minute candles
    period_in_units_from_trade: i64,
}

impl AlignedTimeRule {
    /// Create a new instance of the aligned time rule,
    /// with a given candle period in seconds
    ///
    /// # Arguments:
    /// period_s: How many seconds a candle will contain
    /// ts_res: The resolution each Trade timestamp will have
    ///
    pub fn new(
        period_ms: MillisecondPeriod,
        trade_timestamp_resolution: TimestampResolution,
    ) -> Self {
        use TimestampResolution::*;
        let ts_multiplier = match trade_timestamp_resolution {
            Millisecond => 1,
            Microsecond => 1_000,
            Nanosecond => 1_000_000,
        };

        Self {
            reference_timestamp: 0,
            period_in_units_from_trade: period_ms.get() as i64 * ts_multiplier,
        }
    }

    /// Calculates the "aligned" timestamp, which the rule will use when receiving
    /// for determining the trigger.  This is done at the initialization of
    /// each period.
    #[must_use]
    pub fn aligned_timestamp(&self, timestamp: i64) -> i64 {
        timestamp - (timestamp % self.period_in_units_from_trade)
    }
}

impl<C, T> AggregationRule<C, T> for AlignedTimeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.reference_timestamp == 0 {
            self.reference_timestamp = self.aligned_timestamp(trade.timestamp());
            return false;
        }

        let should_trigger =
            trade.timestamp() - self.reference_timestamp >= self.period_in_units_from_trade;
        if should_trigger {
            // Advance the trigger timestamp to the next period.
            // If the period is too small, then the trade timestamps will out-pace and always trigger.
            // Alternatively doing `self.reference_timestamp = self.aligned_timestamp(trade.timestamp())` will cause drift
            // and not produce enough samples.
            self.reference_timestamp = self.reference_timestamp + self.period_in_units_from_trade
        }

        should_trigger
    }
}

#[cfg(test)]
mod tests {
    use trade_aggregation_derive::Candle;

    use super::*;
    use crate::{
        aggregate_all_trades,
        candle_components::{
            CandleComponent, CandleComponentUpdate, Close, NumTrades, Open, Volume,
        },
        load_trades_from_csv,
        plot::OhlcCandle,
        GenericAggregator, ModularCandle, TimestampResolution, Trade, M1, M15,
    };

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
        close: Close,
        num_trades: NumTrades<u32>,
        volume: Volume,
    }

    #[test]
    fn aligned_time_rule() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<MyCandle, AlignedTimeRule, Trade>::new(
            AlignedTimeRule::new(M15, TimestampResolution::Millisecond),
            false,
            MyCandle::default,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 396);

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
        let trades = load_trades_from_csv("data/Bitstamp_BTCEUR_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<MyCandle, AlignedTimeRule, Trade>::new(
            AlignedTimeRule::new(M1, TimestampResolution::Microsecond),
            false,
            MyCandle::default,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);

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
            AlignedTimeRule::new(M1, TimestampResolution::Millisecond),
            false,
            OhlcCandle::default,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
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
            AlignedTimeRule::new(M1, TimestampResolution::Millisecond),
            false,
            OhlcCandle::default,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].open(), 100.0);
        assert_eq!(candles[0].close(), 101.0);
        assert_eq!(candles[1].open(), 100.5);
        assert_eq!(candles[1].close(), 100.5);
    }
}
