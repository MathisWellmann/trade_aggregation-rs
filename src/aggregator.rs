use std::marker::PhantomData;

use crate::{AggregationRule, ModularCandle, TakerTrade};

/// Defines the needed methods for any online `Aggregator`
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

    /// Get a reference to an unfinished `Candle`.
    /// Accessing a `Candle` using this method does not guarantee that the `AggregationRule` is respected.
    /// It is generally advised to call `update` instead and use the resulting `Candle` if its `Some`.
    fn unfinished_candle(&self) -> &Candle;
}

/// An `Aggregator` that is generic over
/// the type of Candle being produced,
/// as well as by which rule the candle is created
#[derive(Debug, Clone)]
pub struct GenericAggregator<C, R, T> {
    candle: C,
    aggregation_rule: R,
    trade_type: PhantomData<T>,
}

impl<C, R, T> GenericAggregator<C, R, T>
where
    C: ModularCandle<T>,
    R: AggregationRule<C, T>,
    T: TakerTrade,
{
    /// Create a new instance with a concrete aggregation rule
    /// and a default candle
    pub fn new(aggregation_rule: R) -> Self {
        Self {
            candle: Default::default(),
            aggregation_rule,
            trade_type: PhantomData,
        }
    }
}

impl<C, R, T> Aggregator<C, T> for GenericAggregator<C, R, T>
where
    C: ModularCandle<T>,
    R: AggregationRule<C, T>,
    T: TakerTrade,
{
    fn update(&mut self, trade: &T) -> Option<C> {
        if self.aggregation_rule.should_trigger(trade, &self.candle) {
            let candle = self.candle.clone();
            self.candle.reset();
            // Also include the initial information in the candle.
            // This means the trade data at the boundary is included twice.
            // This especially ensures `Open` and `Close` values are correct.
            // TODO: If this behaviour of including trade info at the boundary in both candles,
            // A flag may be added to specify the exact behaviour.
            self.candle.update(trade);
            return Some(candle);
        }

        self.candle.update(trade);
        None
    }

    fn unfinished_candle(&self) -> &C {
        &self.candle
    }
}

#[cfg(test)]
mod tests {
    use trade_aggregation_derive::Candle;

    use super::*;
    use crate::{
        candle_components::{CandleComponent, CandleComponentUpdate, Close, NumTrades, Open},
        load_trades_from_csv, ModularCandle, TimeRule, TimestampResolution, Trade, M1,
    };

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
        close: Close,
        num_trades: NumTrades<u32>,
    }

    #[test]
    fn generic_aggregator() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv")
            .expect("Could not load trades from file!");

        let rule = TimeRule::new(M1, TimestampResolution::Millisecond);
        let mut a = GenericAggregator::<MyCandle, TimeRule, Trade>::new(rule);

        let mut candle_counter: usize = 0;
        for t in trades.iter() {
            if let Some(_candle) = a.update(t) {
                // println!(
                //     "got candle: {:?} at {:?}, {:?}",
                //     candle, t.timestamp, t.price
                // );

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
