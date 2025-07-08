use crate::{AggregationRule, MillisecondPeriod, ModularCandle, TakerTrade, TimestampResolution};

/// The classic time based aggregation rule,
/// creating a new candle every n seconds
#[derive(Debug, Clone)]
pub struct TimeRule {
    // The timestamp this rule uses as a reference
    // in the unit of the incoming trades.
    reference_timestamp: i64,

    // The period for the candle in the timestamp resolution of the candle as provided in the constructor.
    period_in_units_from_trade: i64,
}

impl TimeRule {
    /// Create a new instance of the time rule,
    /// with a given candle period in seconds
    ///
    /// # Arguments:
    /// `period_ms`: How many milliseconds a candle will contain.
    /// `trade_timestamp_resolution`: The resolution each Trade timestamp will have
    ///
    pub fn new(
        period_ms: MillisecondPeriod,
        trade_timestamp_resolution: TimestampResolution,
    ) -> Self {
        use TimestampResolution::*;
        // Given the timestamp resolution of the trades, a certain multiplier is required to compute the number of units for the sample period.
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
}

impl<C, T> AggregationRule<C, T> for TimeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.reference_timestamp == 0 {
            self.reference_timestamp = trade.timestamp();
        }
        let should_trigger =
            trade.timestamp() - self.reference_timestamp > self.period_in_units_from_trade;
        if should_trigger {
            // Advance the trigger timestamp to the next period.
            // If the period is too small, then the trade timestamps will out-pace and always trigger.
            // Alternatively doing `self.reference_timestamp = trade.timestamp()` will cause drift
            // and not produce enough samples.
            self.reference_timestamp = self.reference_timestamp + self.period_in_units_from_trade;
        }

        should_trigger
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_all_trades, load_trades_from_csv,
        plot::{plot_ohlc_candles, OhlcCandle},
        GenericAggregator, Trade, H1, M15, M5,
    };

    #[test]
    fn time_candles_plot() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M15, TimestampResolution::Millisecond),
            false,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        println!("got {} candles", candles.len());

        plot_ohlc_candles(&candles, "img/time_candles_plot.png", (2560, 1440)).unwrap();
    }

    #[test]
    fn time_rule_differing_periods() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M15, TimestampResolution::Millisecond),
            false,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 396);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M5, TimestampResolution::Millisecond),
            false,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 1190);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(H1, TimestampResolution::Millisecond),
            false,
        );
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        assert_eq!(candles.len(), 99);
    }

    #[test]
    fn time_rule_differing_timestamp_resolutions() {
        // We know the XBTUSD series from Bitmex has millisecond timestamp resolution
        let trades_ms = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        // we can therefore transform them into seconds, microseconds and nanoseconds respectively
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

        // And make sure they produce the same number of candles given the differing timestamp resolutions.
        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M15, TimestampResolution::Millisecond),
            false,
        );
        let candles = aggregate_all_trades(&trades_ms, &mut aggregator);
        assert_eq!(candles.len(), 396);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M15, TimestampResolution::Microsecond),
            false,
        );
        let candles = aggregate_all_trades(&trades_micros, &mut aggregator);
        assert_eq!(candles.len(), 396);

        let mut aggregator = GenericAggregator::<OhlcCandle, TimeRule, Trade>::new(
            TimeRule::new(M15, TimestampResolution::Nanosecond),
            false,
        );
        let candles = aggregate_all_trades(&trades_ns, &mut aggregator);
        assert_eq!(candles.len(), 396);
    }
}
