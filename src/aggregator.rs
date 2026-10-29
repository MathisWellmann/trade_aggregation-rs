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

/// Which candle receives the trade that triggers the `AggregationRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerTrade {
    /// The trade opens the next candle and is not part of the candle it closes.
    /// Every trade is counted once.
    /// The close of a candle can differ from the open of the next one,
    /// as with time candles on Tradingview.
    NextCandle,
    /// The trade is the last trade of the candle it closes.
    /// The next candle opens with the trade after it.
    /// Every trade is counted once and the close of a candle is the price of the triggering trade.
    ClosingCandle,
    /// The trade is the last trade of the candle it closes and the first trade of the next candle,
    /// so the close of a candle equals the open of the next one.
    /// Components that sum over trades (e.g. `Volume`, `NumTrades`) count this trade twice.
    Both,
}

/// An `Aggregator` that is generic over
/// the type of Candle being produced,
/// as well as by which rule the candle is created
#[derive(Debug, Clone)]
pub struct GenericAggregator<C, R, T> {
    candle: C,
    aggregation_rule: R,
    trigger_trade: TriggerTrade,
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
    ///     With `true`, components that sum over trades (e.g. `Volume`, `NumTrades`) count the triggering trade twice.
    ///     `false` is [`TriggerTrade::NextCandle`] and `true` is [`TriggerTrade::Both`].
    ///     Use [`Self::with_trigger_trade`] for [`TriggerTrade::ClosingCandle`], which counts every trade once.
    pub fn new(aggregation_rule: R, include_trade_that_triggered_rule: bool) -> Self {
        let trigger_trade = if include_trade_that_triggered_rule {
            TriggerTrade::Both
        } else {
            TriggerTrade::NextCandle
        };
        Self::with_trigger_trade(aggregation_rule, trigger_trade)
    }

    /// Create a new instance with a concrete aggregation rule
    /// and an empty candle.
    ///
    /// # Arguments:
    /// `aggregation_rule`: The rule that dictates when to trigger the creation of a new candle.
    /// `trigger_trade`: Which candle receives the trade that triggers the rule.
    pub fn with_trigger_trade(aggregation_rule: R, trigger_trade: TriggerTrade) -> Self {
        Self {
            candle: Default::default(),
            aggregation_rule,
            trigger_trade,
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
            if matches!(
                self.trigger_trade,
                TriggerTrade::ClosingCandle | TriggerTrade::Both
            ) {
                self.candle.update(trade);
            }
            let candle = self.candle.clone();

            // Create a new candle.
            self.candle.reset();
            if matches!(
                self.trigger_trade,
                TriggerTrade::NextCandle | TriggerTrade::Both
            ) {
                self.candle.update(trade);
            }

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
        candle_components::{
            CandleComponent, CandleComponentUpdate, Close, NumTrades, Open, Volume,
        },
        load_trades_from_csv, ModularCandle, TickRule, TimeRule, TimestampResolution, Trade, M1,
    };

    #[derive(Default, Debug, Clone, Candle)]
    struct CountingCandle {
        open: Open,
        close: Close,
        volume: Volume,
        num_trades: NumTrades<u32>,
    }

    /// Ten trades with prices 1..=10 and sizes 1..=10, alternating sides.
    fn ten_trades() -> Vec<Trade> {
        (1..=10)
            .map(|i| Trade {
                timestamp: i,
                price: i as f64,
                size: if i % 2 == 0 { i as f64 } else { -(i as f64) },
            })
            .collect()
    }

    /// Aggregate `ten_trades` into candles of three trades and return the
    /// finished candles and the unfinished one.
    fn aggregate_ten(trigger_trade: TriggerTrade) -> (Vec<CountingCandle>, CountingCandle) {
        let mut a = GenericAggregator::<CountingCandle, TickRule, Trade>::with_trigger_trade(
            TickRule::new(3),
            trigger_trade,
        );
        let candles: Vec<CountingCandle> =
            ten_trades().iter().filter_map(|t| a.update(t)).collect();
        (candles, a.unfinished_candle().clone())
    }

    fn total_volume(candles: &[CountingCandle], unfinished: &CountingCandle) -> f64 {
        candles.iter().map(|c| c.volume()).sum::<f64>() + unfinished.volume()
    }

    fn total_trades(candles: &[CountingCandle], unfinished: &CountingCandle) -> u32 {
        candles.iter().map(|c| c.num_trades()).sum::<u32>() + unfinished.num_trades()
    }

    #[test]
    fn trigger_trade_next_candle_counts_every_trade_once() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::NextCandle);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 10);
        assert_eq!(total_volume(&candles, &unfinished), 55.0);
        // The trigger trades 3, 6 and 9 open the next candle.
        assert_eq!(candles[0].num_trades(), 2);
        assert_eq!(candles[0].close(), 2.0);
        assert_eq!(candles[1].open(), 3.0);
        assert_eq!(candles[1].close(), 5.0);
        assert_eq!(unfinished.open(), 9.0);
    }

    #[test]
    fn trigger_trade_closing_candle_counts_every_trade_once() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::ClosingCandle);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 10);
        assert_eq!(total_volume(&candles, &unfinished), 55.0);
        // The trigger trades 3, 6 and 9 close their candle.
        for (candle, close) in candles.iter().zip([3.0, 6.0, 9.0]) {
            assert_eq!(candle.num_trades(), 3);
            assert_eq!(candle.close(), close);
        }
        assert_eq!(candles[1].open(), 4.0);
        assert_eq!(unfinished.num_trades(), 1);
        assert_eq!(unfinished.open(), 10.0);
    }

    #[test]
    fn trigger_trade_both_counts_the_trigger_trade_twice() {
        let (candles, unfinished) = aggregate_ten(TriggerTrade::Both);
        assert_eq!(candles.len(), 3);
        assert_eq!(total_trades(&candles, &unfinished), 13);
        assert_eq!(total_volume(&candles, &unfinished), 55.0 + 3.0 + 6.0 + 9.0);
        for pair in candles.windows(2) {
            assert_eq!(pair[0].close(), pair[1].open());
        }
        assert_eq!(candles[2].close(), unfinished.open());
    }

    #[test]
    fn new_maps_the_bool_to_the_documented_trigger_trade() {
        let a = GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), false);
        assert_eq!(a.trigger_trade, TriggerTrade::NextCandle);
        let a = GenericAggregator::<CountingCandle, TickRule, Trade>::new(TickRule::new(3), true);
        assert_eq!(a.trigger_trade, TriggerTrade::Both);
    }

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
        let mut a = GenericAggregator::<MyCandle, TimeRule, Trade>::new(rule, false);

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
