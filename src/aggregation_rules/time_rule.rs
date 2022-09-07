use crate::{AggregationRule, ModularCandle, TakerTrade};

/// The resolution of the "TakerTrade" timestamps
#[derive(Debug, Clone, Copy)]
pub enum TimestampResolution {
    /// The timestamp of the TakerTrade is measured in seconds
    Second,

    /// The timestamp of the TakerTrade is measured in milliseconds
    Millisecond,

    /// The timestamp of the TakerTrade is measured in microseconds
    Microsecond,

    /// The timestamp of the TakerTrade is measured in nanoseconds
    Nanosecond,
}

/// The classic time based aggregation rule,
/// creating a new candle every n seconds
pub struct TimeRule {
    /// If true, the reference timestamp needs to be reset
    init: bool,

    // The timestamp this rule uses as a reference
    reference_timestamp: i64,

    // The period for the candle in seconds
    // constants can be used nicely here from constants.rs
    // e.g.: M1 -> 1 minute candles
    period_s: i64,

    // Multiplies each trade timestamp by this much,
    // Used for handling differing timestamp resolutions
    ts_multiplier: i64,
}

impl TimeRule {
    /// Create a new instance of the time rule,
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
            period_s,
            ts_multiplier,
        }
    }
}

impl<C, T> AggregationRule<C, T> for TimeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.init {
            self.reference_timestamp = trade.timestamp();
            self.init = false;
        }
        let should_trigger =
            trade.timestamp() - self.reference_timestamp > self.period_s * self.ts_multiplier;
        if should_trigger {
            self.init = true;
        }

        should_trigger
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        aggregate_all_trades, load_trades_from_csv,
        plot::{plot_ohlc_candles, OhlcCandle},
        GenericAggregator, Trade, H1, M15, M5,
    };

    use super::*;

    #[test]
    fn time_candles_plot() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Millisecond,
        ));
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        println!("got {} candles", candles.len());

        plot_ohlc_candles(&candles, "img/time_candles_plot.png", (2560, 1440)).unwrap();
    }

    #[test]
    fn time_rule_differing_periods() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Millisecond,
        ));
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 395);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M5,
            TimestampResolution::Millisecond,
        ));
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 1180);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            H1,
            TimestampResolution::Millisecond,
        ));
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 99);
    }

    #[test]
    fn time_rule_differing_timestamp_resolutions() {
        // We know the XBTUSD series from Bitmex has millisecond timestamp resolution
        let trades_ms = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        // we can therefore transform them into seconds, microseconds and nanoseconds respectively
        let trades_s: Vec<Trade> = trades_ms
            .iter()
            .map(|v| Trade {
                timestamp: v.timestamp / 1000,
                price: v.price,
                size: v.size,
            })
            .collect();
        let trades_micros: Vec<Trade> = trades_ms
            .iter()
            .map(|v| Trade {
                timestamp: v.timestamp * 1000,
                price: v.price,
                size: v.size,
            })
            .collect();
        let trades_ns: Vec<Trade> = trades_ms
            .iter()
            .map(|v| Trade {
                timestamp: v.timestamp * 1_000_000,
                price: v.price,
                size: v.size,
            })
            .collect();

        // And make sure they produce the same number of candles regardless of timestamp resolution
        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Second,
        ));
        let candles = aggregate_all_trades(&trades_s, &mut aggregator);
        assert_eq!(candles.len(), 395);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Millisecond,
        ));
        let candles = aggregate_all_trades(&trades_ms, &mut aggregator);
        assert_eq!(candles.len(), 395);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Microsecond,
        ));
        let candles = aggregate_all_trades(&trades_micros, &mut aggregator);
        assert_eq!(candles.len(), 395);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(TimeRule::new(
            M15,
            TimestampResolution::Nanosecond,
        ));
        let candles = aggregate_all_trades(&trades_ns, &mut aggregator);
        assert_eq!(candles.len(), 395);
    }
}
