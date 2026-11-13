use crate::{AggregationRule, Error, LogReturn, Result, Signal, ThresholdRule, TradePrice};

/// Closes a candle when a symmetric CUSUM filter on a [`Signal`] fires, by default on the
/// log returns of the trades (Lam and Yam 1997; López de Prado, Advances in Financial Machine
/// Learning, ch. 2.5.2.1).
///
/// With `x` the value of the signal at an event, the rule keeps
/// `s_up = max(0, s_up + x)` and `s_down = min(0, s_down + x)`.
/// It triggers when `s_up >= threshold` or `-s_down >= threshold`, and then restarts both sums.
/// Unlike a [`RelativePriceRule`](crate::RelativePriceRule), small moves against the run
/// do not restart it, and noise around a level does not add up as in a volatility rule:
/// it samples when the price has drifted in one direction.
///
/// # A threshold in standard deviations
///
/// A threshold on raw log returns is a fraction of the price: 1% is a small move for a volatile
/// coin and a large one for a bond future, so one parameter grid does not fit every symbol.
/// López de Prado subtracts the expected value `E[y]` from each observation and measures the
/// threshold in units of the volatility, `h = k * sigma`. A [`Standardized`](crate::Standardized)
/// signal does both online and without lookahead, so that `k` is a number of standard deviations
/// and the same `k` fits every symbol:
///
/// ```
/// use trade_aggregation::{CusumRule, LogReturn, MidPrice, Standardized};
///
/// let k = 4.0;
/// let returns = Standardized::new(LogReturn::new(MidPrice), 0.001, 1_000)?;
/// let _rule = CusumRule::new(k)?.with_signal(returns);
/// # Ok::<(), trade_aggregation::Error>(())
/// ```
///
/// # Signals with a mean
///
/// A signal with a non-zero mean `mu`, e.g. the returns of a trending price or the signed volume
/// of a market with more buying than selling, adds `mu` per value to one of the sums:
/// the filter then fires about every `threshold / |mu|` values, whatever happens.
/// Standardize such signals, which removes their moving mean.
///
/// With [`Adaptive`](crate::Adaptive), use [`Scaling::Diffusive`](crate::Scaling::Diffusive):
/// the sums of values with a zero mean grow with the square root of time.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CusumRule<S = LogReturn<TradePrice>> {
    threshold: f64,
    signal: S,
    s_up: f64,
    s_down: f64,
}

impl CusumRule {
    /// Create a new instance on the log returns of the trade prices.
    ///
    /// # Arguments:
    /// `threshold`: The cumulative log return at which the filter fires, e.g. 0.01 for about 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold` is not positive and finite.
    pub fn new(threshold: f64) -> Result<Self> {
        let mut rule = Self {
            threshold: 1.0,
            signal: LogReturn::new(TradePrice),
            s_up: 0.0,
            s_down: 0.0,
        };
        rule.set_threshold(threshold)?;
        Ok(rule)
    }
}

impl<S> CusumRule<S> {
    /// Accumulate the values of `signal` instead, with the same threshold,
    /// which is then in the unit of the signal, e.g. standard deviations
    /// for a [`Standardized`](crate::Standardized) signal.
    pub fn with_signal<T>(self, signal: T) -> CusumRule<T> {
        CusumRule {
            threshold: self.threshold,
            signal,
            s_up: 0.0,
            s_down: 0.0,
        }
    }

    /// Accumulate the log returns of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    pub fn with_price_source<Q>(self, source: Q) -> CusumRule<LogReturn<Q>> {
        self.with_signal(LogReturn::new(source))
    }
}

impl<E, S: Signal<E>> AggregationRule<E> for CusumRule<S> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(x) = self.signal.observe(event) else {
            return false;
        };
        assert!(x.is_finite(), "signal values must be finite, got {x}");
        self.s_up = (self.s_up + x).max(0.0);
        self.s_down = (self.s_down + x).min(0.0);
        debug_assert!(self.s_up >= 0.0);
        debug_assert!(self.s_down <= 0.0);
        let trigger = self.s_up >= self.threshold || -self.s_down >= self.threshold;
        if trigger {
            self.s_up = 0.0;
            self.s_down = 0.0;
        }
        trigger
    }

    /// Restart both sums. The signal follows the stream and keeps its state,
    /// e.g. the last price stays the reference for the next return.
    fn reset(&mut self) {
        self.s_up = 0.0;
        self.s_down = 0.0;
    }
}

impl<S> ThresholdRule for CusumRule<S> {
    fn threshold(&self) -> f64 {
        self.threshold
    }

    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        self.threshold = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{random_flow, trade, trades_at_prices, trigger_indices},
        MarketEvent, MidPrice, Quote, Standardized, Trade,
    };

    /// Prices that move by the given log returns from 100.
    fn prices_from_returns(returns: &[f64]) -> Vec<f64> {
        let mut price = 100.0;
        let mut prices = vec![price];
        for r in returns {
            price *= r.exp();
            prices.push(price);
        }
        prices
    }

    fn rule(threshold: f64) -> CusumRule {
        CusumRule::new(threshold).expect("valid threshold")
    }

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(CusumRule::new(0.0).is_err());
        assert!(CusumRule::new(-0.01).is_err());
        assert!(CusumRule::new(f64::NAN).is_err());
        assert!(CusumRule::new(f64::INFINITY).is_err());
    }

    #[test]
    fn fires_on_a_drift_despite_small_pullbacks() {
        // Up 0.4%, down 0.1%, repeated: the up sum grows by 0.3% per pair.
        let returns = [0.004, -0.001, 0.004, -0.001, 0.004, -0.001, 0.004];
        let trades = trades_at_prices(&prices_from_returns(&returns));
        // s_up after each trade: 0.4, 0.3, 0.7, 0.6, 1.0 (fires on trade 5), ...
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), vec![5]);
    }

    #[test]
    fn fires_downwards() {
        let trades = trades_at_prices(&prices_from_returns(&[-0.006, 0.001, -0.006]));
        assert_eq!(trigger_indices(&mut rule(0.01), &trades), vec![3]);
    }

    #[test]
    fn noise_around_a_level_does_not_fire() {
        let returns: Vec<f64> = (0..1_000)
            .map(|i| if i % 2 == 0 { 0.005 } else { -0.005 })
            .collect();
        let trades = trades_at_prices(&prices_from_returns(&returns));
        assert!(trigger_indices(&mut rule(0.01), &trades).is_empty());
    }

    #[test]
    fn both_sums_restart_after_firing() {
        // Down 0.8% first, then up 1%: fires on the way up and restarts s_down,
        // so the next 0.3% down does not complete the earlier 0.8%.
        let trades = trades_at_prices(&prices_from_returns(&[-0.008, 0.01, -0.003]));
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), vec![2]);
    }

    #[test]
    fn reset_restarts_the_sums_but_keeps_the_reference_price() {
        let mut r = rule(0.00999);
        let prices = prices_from_returns(&[0.008, 0.002, 0.008, 0.002]);
        let trades = trades_at_prices(&prices);
        assert!(trigger_indices(&mut r, &trades[..2]).is_empty());
        AggregationRule::<Trade>::reset(&mut r);
        // From trade 1 on: 0.2% (trade 2), 1.0% (trade 3).
        assert_eq!(trigger_indices(&mut r, &trades[2..]), vec![1]);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(0.01);
        r.set_threshold(0.02).expect("valid threshold");
        assert_eq!(r.threshold(), 0.02);
        assert!(r.set_threshold(-1.0).is_err());
        assert_eq!(r.threshold(), 0.02);
    }

    type Event = MarketEvent<Trade, Quote>;

    /// A quote with a spread of 1 around `mid`.
    fn quote_at(mid: f64) -> Event {
        MarketEvent::Quote(Quote {
            timestamp: 0,
            bid_price: mid - 0.5,
            bid_size: 1.0,
            ask_price: mid + 0.5,
            ask_size: 1.0,
        })
    }

    /// The log returns of a random walk with steps of `step`.
    fn random_returns(n: i64, step: f64) -> Vec<f64> {
        random_flow(n, 0.5, 42)
            .iter()
            .map(|t| step * t.size)
            .collect()
    }

    fn standardized_rule<P>(k: f64, source: P) -> CusumRule<Standardized<LogReturn<P>>> {
        let returns = Standardized::new(LogReturn::new(source), 0.1, 10).expect("valid parameters");
        rule(k).with_signal(returns)
    }

    #[test]
    fn a_drifting_signal_fires_regularly_unless_standardized() {
        // Up 0.2% every other trade: a mean of 0.1% per trade.
        let returns: Vec<f64> = (0..100)
            .map(|i| if i % 2 == 0 { 0.002 } else { 0.0 })
            .collect();
        let trades = trades_at_prices(&prices_from_returns(&returns));
        let every_ten: Vec<usize> = (1..=10).map(|i| 10 * i - 1).collect();
        assert_eq!(trigger_indices(&mut rule(0.00999), &trades), every_ten);
        // Without the mean, the values alternate around +-1 standard deviation.
        let mut standardized = standardized_rule(2.0, TradePrice);
        assert!(trigger_indices(&mut standardized, &trades).is_empty());
    }

    #[test]
    fn a_threshold_in_standard_deviations_does_not_depend_on_the_volatility() {
        let calm = trades_at_prices(&prices_from_returns(&random_returns(2_000, 0.001)));
        let wild = trades_at_prices(&prices_from_returns(&random_returns(2_000, 0.01)));
        let raw_calm = trigger_indices(&mut rule(0.02), &calm);
        let raw_wild = trigger_indices(&mut rule(0.02), &wild);
        assert!(raw_calm.len() * 10 < raw_wild.len());
        let sd_calm = trigger_indices(&mut standardized_rule(3.0, TradePrice), &calm);
        let sd_wild = trigger_indices(&mut standardized_rule(3.0, TradePrice), &wild);
        assert!(sd_calm.len() > 10);
        assert_eq!(sd_calm, sd_wild);
    }

    #[test]
    fn with_price_source_follows_the_mid_price_and_ignores_trades() {
        let mids = prices_from_returns(&[0.004, -0.001, 0.004, -0.001, 0.004, -0.001, 0.004]);
        // Trades far away from the mid price would fire a CUSUM on the trade prices.
        let events: Vec<Event> = mids
            .iter()
            .zip(0..)
            .flat_map(|(mid, i)| {
                let price = if i % 2 == 0 { 50.0 } else { 200.0 };
                [quote_at(*mid), MarketEvent::Trade(trade(0, price, 1.0))]
            })
            .collect();
        let mut r = rule(0.00999).with_price_source(MidPrice);
        assert_eq!(r.threshold(), 0.00999);
        let fired: Vec<usize> = (0..events.len())
            .filter(|i| r.should_trigger(&events[*i]))
            .collect();
        // As in `fires_on_a_drift_despite_small_pullbacks`, on the quote of the fifth return.
        assert_eq!(fired, vec![10]);
    }

    #[test]
    fn a_standardized_mid_price_ignores_trades() {
        let mids = prices_from_returns(&random_returns(1_000, 0.001));
        let quotes: Vec<Event> = mids.iter().map(|mid| quote_at(*mid)).collect();
        let events: Vec<Event> = quotes
            .iter()
            .flat_map(|q| [*q, MarketEvent::Trade(trade(0, 1.0, -1.0))])
            .collect();
        let fired = |events: &[Event]| -> Vec<usize> {
            let mut r = standardized_rule(3.0, MidPrice);
            (0..events.len())
                .filter(|i| r.should_trigger(&events[*i]))
                .collect()
        };
        let on_quotes = fired(&quotes);
        assert!(!on_quotes.is_empty());
        let on_events = fired(&events);
        assert!(on_events.iter().all(|i| i % 2 == 0), "only quotes fire");
        let on_events: Vec<usize> = on_events.iter().map(|i| i / 2).collect();
        assert_eq!(on_events, on_quotes);
    }
}
