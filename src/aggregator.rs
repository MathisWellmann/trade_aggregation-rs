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
    // During some aggregations, the desired behaviour is that the trade that crosses the trigger boundary
    // is included in both the current and next candle.
    // Examples uses include ensuring the close and open price of the current and next candle are equal.
    // If that's desired, set the field to true during construction of `Self`.
    include_trade_that_triggered_rule: bool,
    _trade_type: PhantomData<T>,
}

impl<C, R, T> GenericAggregator<C, R, T>
where
    C: ModularCandle<T>,
    R: AggregationRule<C, T>,
    T: TakerTrade,
{
    /// Create a new instance with a concrete aggregation rule
    /// and an empty candle.
    ///
    /// # Arguments:
    /// `aggregation_rule`: The rule that dictates when to trigger the creation of a new candle.
    /// `include_trade_that_triggered_rule`: If true, the trade that triggered a rule is included in the current candle
    ///     as well as the next one.
    ///     During some aggregations, the desired behaviour is that the trade that crosses the trigger boundary
    ///     is included in both the current and next candle.
    ///     Examples uses include ensuring the close and open price of the current and next candle are equal.
    ///     If that's desired, set the field to true during construction of `Self`.
    ///     E.g on Tradingview the time aggregation would have this set to `false`, which may create gaps between close and open of subsequent candles.
    /// `init_candle`: this is a zero argument closure which is used to initialize the candle that is being built up by
    ///     the aggregator until it is triggered.  This allows users to embed non-default state into the aggregation process, such
    ///     as tick size for binning aggregators.
    pub fn new<F: Fn() -> C>(
        aggregation_rule: R,
        include_trade_that_triggered_rule: bool,
        init_candle: F,
    ) -> Self {
        Self {
            candle: init_candle(),
            aggregation_rule,
            include_trade_that_triggered_rule,
            _trade_type: PhantomData,
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
            // During some aggregations, the desired behaviour is that the trade that crosses the trigger boundary
            // is included in both the current and next candle.
            // Examples uses include ensuring the close and open price of the current and next candle are equal.
            // If that's desired, set the field to true during construction of `Self`.
            if self.include_trade_that_triggered_rule {
                self.candle.update(trade);
            }
            let candle = self.candle.clone();

            // Create a new candle.
            self.candle.reset();
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
        let mut a =
            GenericAggregator::<MyCandle, TimeRule, Trade>::new(rule, false, MyCandle::default);

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
        assert_eq!(candle_counter, 5953);
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
