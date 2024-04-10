use crate::{aggregation_rules::TimestampResolution, AggregationRule, ModularCandle, TakerTrade};

/// The classic time based aggregation rule,
/// creating a new candle every n seconds.  The time trigger is aligned such that
/// the trigger points are starting from a time equals zero.  For example, if the first
/// tick comes in a 1:32:00 on a 5 minute candle, that first candle will only contain
/// 3 minutes of trades, representing a 1:30 start.
#[derive(Debug, Clone)]
pub struct AlignedTimeRule {
    /// If true, the reference timestamp needs to be reset
    init: bool,

    // The timestamp this rule uses as a reference
    reference_timestamp: i64,

    // The period for the candle in seconds
    // constants can be used nicely here from constants.rs
    // e.g.: M1 -> 1 minute candles
    period_s: i64,
}

impl AlignedTimeRule {
    /// Create a new instance of the aligned time rule,
    /// with a given candle period in seconds
    ///
    /// # Arguments:
    /// period_s: How many seconds a candle will contain
    /// ts_res: The resolution each Trade timestamp will have
    ///
    pub fn new(period_s: i64, ts_res: TimestampResolution) -> Self {
        let ts_multiplier = match ts_res {
            TimestampResolution::Second => 1,
            TimestampResolution::Millisecond => 1_000,
            TimestampResolution::Microsecond => 1_000_000,
            TimestampResolution::Nanosecond => 1_000_000_000,
        };

        Self {
            init: true,
            reference_timestamp: 0,
            period_s: period_s * ts_multiplier,
        }
    }

    /// Calculates the "aligned" timestamp, which the rule will use when receiving
    /// for determining the trigger.  This is done at the initialization of
    /// each period.
    #[must_use]
    pub fn aligned_timestamp(&self, timestamp: i64) -> i64 {
        timestamp - (timestamp % self.period_s)
    }
}

impl<C, T> AggregationRule<C, T> for AlignedTimeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.init {
            self.reference_timestamp = self.aligned_timestamp(trade.timestamp());
            self.init = false;
        }
        let should_trigger = trade.timestamp() - self.reference_timestamp > self.period_s;
        if should_trigger {
            self.init = true;
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
        load_trades_from_csv, GenericAggregator, ModularCandle, TimestampResolution, Trade, M1,
        M15,
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
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);

        let c = &candles[0];
        assert_eq!(c.num_trades(), 10);
        assert_eq!(c.volume(), 0.27458132);
    }
}
