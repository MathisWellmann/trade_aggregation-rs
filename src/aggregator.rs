use crate::Trade;

/// Defines the needed methods for any online aggregator
pub trait Aggregator<Candle> {
    /// Updates the aggregation state with a new trade
    ///
    /// # Arguments:
    /// trade: the trade information to add to the aggregation process
    ///
    /// # Returns:
    /// Some output only when a new candle has been created,
    /// otherwise it returns None
    fn update(&mut self, trade: &Trade) -> Option<Candle>;
}

/// Defines under what conditions one aggregation period is finished
pub trait AggregationRule<C> {
    /// The main method defining when the aggregation is done
    ///
    /// # Arguments:
    /// trade: The most recent taker trade (tick) information
    /// candle: Some generic Candle, allowing for information driven decision making
    ///
    /// # Returns:
    /// if true, the aggregation period is finished and a Candle can be emitted
    /// else the aggregation needs to continue
    fn should_trigger(&mut self, trade: &Trade, candle: &C) -> bool;
}

/// An aggregator that is generic over
/// the type of Candle being produced,
/// as well as by which rule the candle is created
#[derive(Debug, Clone)]
pub struct GenericAggregator<C, R> {
    candle: C,
    aggregation_rule: R,
}

impl<C, R> GenericAggregator<C, R>
where
    C: ModularCandle,
    R: AggregationRule<C>,
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

impl<C, R> Aggregator<C> for GenericAggregator<C, R>
where
    C: ModularCandle,
    R: AggregationRule<C>,
{
    fn update(&mut self, trade: &Trade) -> Option<C> {
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

/// The classic time based aggregation rule,
/// creating a new candle every n milliseconds
pub struct TimeRule {
    /// If true, the reference timestamp needs to be reset
    init: bool,

    // The timestamp this rule uses as a reference
    reference_timestamp: i64,

    // The period for the candle in seconds
    // constants can be used nicely here from constants.rs
    // e.g.: M1 -> 1 minute candles
    period_s: i64,
}

impl TimeRule {
    /// Create a new instance of the time rule,
    /// with a given candle period in seconds
    pub fn new(period_s: i64) -> Self {
        Self {
            init: true,
            reference_timestamp: 0,
            period_s,
        }
    }
}

impl<C> AggregationRule<C> for TimeRule
where
    C: ModularCandle,
{
    fn should_trigger(&mut self, trade: &Trade, _candle: &C) -> bool {
        if self.init {
            self.reference_timestamp = trade.timestamp;
            self.init = false;
        }
        let should_trigger = trade.timestamp - self.reference_timestamp > self.period_s * 1000;
        if should_trigger {
            self.init = true;
        }

        should_trigger
    }
}

/// A modular candle that can be composed of multiple components
pub trait ModularCandle: Clone + Default {
    /// Updates the candle information with trade information
    fn update(&mut self, trade: &Trade);

    /// Resets the state of the candle
    fn reset(&mut self);
}

/// Each component of a Candle must fullfill this trait
pub trait CandleComponent {
    /// The current value of the component
    // TODO: make output type generic
    fn value(&self) -> f64;

    /// Updates the state with newest trade information
    fn update(&mut self, trade: &Trade);

    /// Resets the component state to its default
    fn reset(&mut self);
}

#[cfg(test)]
mod tests {
    use crate::{
        candle_components::{Close, Open},
        load_trades_from_csv, M1,
    };
    use trade_aggregation_derive::Candle;

    use super::*;

    #[derive(Default, Debug, Clone, Candle)]
    struct MyCandle {
        open: Open,
    }

    #[test]
    fn generic_aggregator() {
        /*
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
        */
    }

    #[test]
    fn candle_macro() {
        let my_candle = MyCandle::default();
        println!("my_candle: {:?}", my_candle);
        println!("open: {}", my_candle.open());
    }
}
