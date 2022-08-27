use crate::{AggregationRule, ModularCandle, TakerTrade, Trade};

/// Defines the needed methods for any online aggregator
pub trait Aggregator<Candle, T: TakerTrade> {
    /// Updates the aggregation state with a new trade
    ///
    /// # Arguments:
    /// trade: the trade information to add to the aggregation process
    ///
    /// # Returns:
    /// Some output only when a new candle has been created,
    /// otherwise it returns None
    fn update(&mut self, trade: &T) -> Option<Candle>;
}

/// An aggregator that is generic over
/// the type of Candle being produced,
/// as well as by which rule the candle is created
#[derive(Debug, Clone)]
pub struct GenericAggregator<C, R> {
    candle: C,
    aggregation_rule: R,
}

impl<C, R, T> GenericAggregator<C, R>
where
    C: ModularCandle<TradeType = T>,
    R: AggregationRule<C, T>,
    T: TakerTrade,
{
    /// Create a new instance with a concrete aggregation rule
    /// and a default candle
    pub fn new(aggregation_rule: R) -> Self {
        Self {
            candle: Default::default(),
            aggregation_rule,
        }
    }
}

impl<C, R, T> Aggregator<C, T> for GenericAggregator<C, R>
where
    C: ModularCandle<TradeType = T>,
    R: AggregationRule<C, T>,
    T: TakerTrade,
{
    fn update(&mut self, trade: &T) -> Option<C> {
        self.candle.update(trade);

        if self.aggregation_rule.should_trigger(trade, &self.candle) {
            let candle = self.candle.clone();
            self.candle.reset();

            Some(candle)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        candle_components::{CandleComponent, Close, Open},
        load_trades_from_csv, ModularCandle, TimeRule, M1,
    };
    use trade_aggregation_derive::Candle;

    use super::*;

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
        close: Close,
    }

    #[test]
    fn generic_aggregator() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
            .expect("Could not load trades from file!");

        let rule = TimeRule::new(M1);
        let mut a = GenericAggregator::<MyCandle, TimeRule>::new(rule);

        let mut candle_counter: usize = 0;
        for t in trades.iter() {
            if let Some(candle) = a.update(t) {
                println!("got candle: {:?}", candle);
                candle_counter += 1;
            }
        }
        assert_eq!(candle_counter, 5704);
    }

    #[test]
    fn candle_macro() {
        let my_candle = MyCandle::default();
        println!("my_candle: {:?}", my_candle);

        // make sure the 'open' and 'close' getters have been generated
        println!("open: {}", my_candle.open());
        println!("close: {}", my_candle.close());
    }
}
