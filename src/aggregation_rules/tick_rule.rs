use crate::{AggregationRule, ModularCandle, TakerTrade};

/// Creates candles every n ticks
#[derive(Debug, Clone)]
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

impl<C, T> AggregationRule<C, T> for TickRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, _trade: &T, _candle: &C) -> bool {
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
        let candles = aggregate_all_trades(&trades, &mut aggregator);
        // As there are 1 million trades in the test data, this will create 1000 candles
        assert_eq!(candles.len(), 1000);
    }
}
