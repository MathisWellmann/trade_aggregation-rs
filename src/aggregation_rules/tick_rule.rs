use crate::{AggregationRule, AsTrade, Error, Result, ThresholdRule};

/// Creates candles every n ticks
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TickRule {
    init: bool,
    tick_counter: usize,
    n_ticks: usize,
}

impl TickRule {
    /// Create a new instance of the `TickRule`
    ///
    /// # Arguments:
    /// `n_ticks`: create a candle every n ticks
    ///
    pub fn new(n_ticks: usize) -> Self {
        Self {
            init: true,
            tick_counter: 0,
            n_ticks,
        }
    }
}

impl<E: AsTrade> AggregationRule<E> for TickRule {
    fn should_trigger(&mut self, event: &E) -> bool {
        if event.as_trade().is_none() {
            return false;
        }
        if self.init {
            self.tick_counter = 0;
            self.init = false;
        }

        self.tick_counter += 1;

        if self.tick_counter >= self.n_ticks {
            self.init = true;
            return true;
        }
        false
    }

    fn reset(&mut self) {
        self.init = true;
    }
}

impl ThresholdRule for TickRule {
    fn threshold(&self) -> f64 {
        self.n_ticks as f64
    }

    /// Rounds `threshold` to whole trades, at least one.
    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        // `as` saturates for thresholds beyond `usize::MAX`.
        self.n_ticks = (threshold.round() as usize).max(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_all_trades, load_trades_from_csv, plot::OhlcCandle, GenericAggregator, Trade,
    };

    #[test]
    fn tick_rule() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();

        let mut aggregator =
            GenericAggregator::<OhlcCandle, TickRule, Trade>::new(TickRule::new(1000), false);
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        // As there are 1 million trades in the test data, this will create 1000 candles
        assert_eq!(candles.len(), 1000);
    }
}
