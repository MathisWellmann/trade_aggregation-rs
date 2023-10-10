use std::fs::File;

use crate::{errors::Result, Aggregator, ModularCandle, TakerTrade, Trade};

/// Determine the candle volume which produces the same number of candles
/// as the given time aggregation equivalent
///
/// # Parameters:
/// total_volume - sum of traded volume over entire time period
/// total_time_days - total number of days
/// target_time_minutes - time aggregated candle period which to target
///
/// # Returns:
/// target candle volume for which volume aggregation produces
/// the same number of candles as the time aggregation did
/// e.g.:
/// 10 days of 1h candle -> 240 candles
/// assuming 9840 volume traded over 10 days
/// -> each candle should have 41 volume to produce 240 candles using volume aggregation
pub fn candle_volume_from_time_period(
    total_volume: f64,
    total_time_days: f64,
    target_time_minutes: f64,
) -> f64 {
    let num_candles = total_time_days * 24.0 * (60.0 / target_time_minutes);
    total_volume / num_candles
}

/// Apply an aggregator for all trades at once
///
/// # Arguments:
/// trades: The input trade data to aggregate
/// aggregator: Something that can aggregate
///
/// # Returns:
/// A vector of aggregated candle data
pub fn aggregate_all_trades<A, C, T>(trades: &[T], aggregator: &mut A) -> Vec<C>
where
    A: Aggregator<C, T>,
    C: ModularCandle<T>,
    T: TakerTrade,
{
    let mut out: Vec<C> = vec![];

    for t in trades {
        if let Some(candle) = aggregator.update(t) {
            out.push(candle);
        }
    }

    out
}

/// Load trades from csv file
///
/// # Arguments:
/// filename: The path to the csv file
///
/// # Returns
/// If Ok, A vector of the trades inside the file
pub fn load_trades_from_csv(filename: &str) -> Result<Vec<Trade>> {
    let f = File::open(filename)?;

    let mut r = csv::Reader::from_reader(f);

    let mut out: Vec<Trade> = vec![];
    for record in r.records() {
        let row = record?;

        let ts = row[0].parse::<i64>()?;
        let price = row[1].parse::<f64>()?;
        let size = row[2].parse::<f64>()?;

        // convert to Trade
        let trade = Trade {
            timestamp: ts,
            price,
            size,
        };
        out.push(trade);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use round::round;

    use crate::{
        candle_components::{Close, High, Low, Open},
        By, CandleComponent, CandleComponentUpdate, GenericAggregator, VolumeRule,
    };

    use super::*;

    #[derive(Debug, Default, Clone)]
    struct MyCandle {
        open: Open,
        high: High,
        low: Low,
        close: Close,
    }

    impl ModularCandle<Trade> for MyCandle {
        fn update(&mut self, trade: &Trade) {
            self.open.update(trade);
            self.high.update(trade);
            self.low.update(trade);
            self.close.update(trade);
        }
        fn reset(&mut self) {
            self.open.reset();
            self.high.reset();
            self.low.reset();
            self.close.reset();
        }
    }

    #[test]
    #[ignore = "Long running test, only run manually"]
    fn test_aggregate_all_trades() {
        type C = MyCandle;
        type T = Trade;
        type GA = GenericAggregator<C, VolumeRule, T>;
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();
        let rule = VolumeRule::new(100.0, By::Quote).unwrap();
        let mut aggregator = GenericAggregator::new(rule);
        let candles = aggregate_all_trades::<GA, C, T>(&trades, &mut aggregator);
        assert!(!candles.is_empty());
    }

    #[test]
    fn test_candle_volume_from_time_period() {
        let total_volume = 100.0;
        let time_days = 10.0;
        let target_time_minutes = 5.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.035);

        let total_volume = 100.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.069);

        let total_volume = 200.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.139);

        let total_volume = 50.0;
        let time_days = 10.0;
        let target_time_minutes = 10.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.035);

        let total_volume = 100.0;
        let time_days = 5.0;
        let target_time_minutes = 5.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.069);

        let total_volume = 100.0;
        let time_days = 5.0;
        let target_time_minutes = 10.0;
        let vol_threshold =
            candle_volume_from_time_period(total_volume, time_days, target_time_minutes);
        assert_eq!(round(vol_threshold, 3), 0.139);
    }
}
