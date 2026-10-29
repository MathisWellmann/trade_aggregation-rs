use crate::{
    aggregation_rules::flow_bars::{check_ticks, Ewma},
    AggregationRule, FlowBarParams, FlowSignal, Result, TakerTrade, ThresholdRule,
};

/// Creates the tick, volume or dollar imbalance bars of López de Prado
/// (Advances in Financial Machine Learning, ch. 2.3.2.1 and 2.3.2.2).
///
/// With `b` the side of a trade (+1 buy, -1 sell) and `x` its contribution
/// (1 for ticks, the size for volume), a bar of `T` trades has the imbalance `theta = sum b * x`.
/// It closes at the first trade where
/// `|theta| >= E[T] * max(|E[b * x]|, min_flow_fraction * E[|x|])`:
/// the net flow exceeds what the expected bar length and the expected imbalance per trade predict.
/// `E[b * x]` and `E[|x|]` are moving averages over trades; `E[T]` is the expected bar length,
/// see [`FlowBarParams`].
///
/// A one-sided flow that persists produces bars of about `E[T]` trades,
/// and a change towards more one-sided flow than expected closes bars early,
/// so bars sample when informed traders arrive.
/// In a balanced market the expected imbalance is small and the floor sets the bar length:
/// about `(min_flow_fraction * E[T])^2` trades.
///
/// The bars during the first `1 / trade_alpha` trades depend on the first trades,
/// as the moving averages start there.
/// [`ThresholdRule`] reads and sets `E[T]`, so [`Adaptive`](crate::Adaptive) can calibrate it
/// to a target duration; leave [`FlowBarParams::with_bar_alpha`] unset then,
/// or two controllers fight over `E[T]`.
#[derive(Debug, Clone)]
pub struct ImbalanceRule {
    signal: FlowSignal,
    params: FlowBarParams,
    expected_ticks: f64,
    expected_imbalance: Ewma,
    expected_magnitude: Ewma,
    imbalance: f64,
    ticks: f64,
}

impl ImbalanceRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `signal`: What each trade contributes: 1 (tick imbalance bars) or its size (volume or dollar imbalance bars).
    /// `params`: The expected bar length and how the expectations are learned.
    pub fn new(signal: FlowSignal, params: FlowBarParams) -> Self {
        Self {
            signal,
            params,
            expected_ticks: params.expected_ticks(),
            expected_imbalance: Ewma::new(params.trade_alpha()),
            expected_magnitude: Ewma::new(params.trade_alpha()),
            imbalance: 0.0,
            ticks: 0.0,
        }
    }

    /// The expected number of trades per bar, `E[T]`.
    pub fn expected_ticks(&self) -> f64 {
        self.expected_ticks
    }

    /// The imbalance the current bar needs to close.
    fn threshold(&self) -> f64 {
        let floor = self.params.min_flow_fraction() * self.expected_magnitude.get();
        self.expected_ticks * self.expected_imbalance.get().abs().max(floor)
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for ImbalanceRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        let flow = self.signal.flow(trade);
        let signed = flow.side * flow.magnitude;
        self.expected_imbalance.update(signed);
        self.expected_magnitude.update(flow.magnitude);
        self.imbalance += signed;
        self.ticks += 1.0;

        let threshold = self.threshold();
        debug_assert!(threshold >= 0.0);
        // Without any flow so far there is nothing to compare against.
        let trigger = threshold > 0.0 && self.imbalance.abs() >= threshold;
        if trigger {
            self.expected_ticks = self
                .params
                .next_expected_ticks(self.expected_ticks, self.ticks);
            self.imbalance = 0.0;
            self.ticks = 0.0;
        }
        debug_assert!(self.expected_ticks >= 1.0);
        trigger
    }

    /// Start a new bar; the expectations stay.
    fn reset(&mut self) {
        self.imbalance = 0.0;
        self.ticks = 0.0;
    }
}

impl ThresholdRule for ImbalanceRule {
    /// The expected number of trades per bar, `E[T]`.
    fn threshold(&self) -> f64 {
        self.expected_ticks
    }

    /// Set the expected number of trades per bar, which must be finite and at least 1.
    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        check_ticks(threshold)?;
        self.expected_ticks = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{random_flow, trade, trigger_indices},
        plot::OhlcCandle,
        By, Trade,
    };

    /// Trades of size 1 on the given sides (`true` = buy).
    fn sided(buys: &[bool]) -> Vec<Trade> {
        buys.iter()
            .zip(0..)
            .map(|(buy, ts)| trade(ts, 100.0, if *buy { 1.0 } else { -1.0 }))
            .collect()
    }

    fn params(expected_ticks: f64, trade_alpha: f64) -> FlowBarParams {
        FlowBarParams::new(expected_ticks)
            .and_then(|p| p.with_trade_alpha(trade_alpha))
            .expect("valid parameters")
    }

    fn mean_bar_length(triggers: &[usize]) -> f64 {
        let first = *triggers.first().expect("some bars");
        let last = *triggers.last().expect("some bars");
        (last - first) as f64 / (triggers.len() - 1) as f64
    }

    #[test]
    fn persistent_buying_closes_bars_of_the_expected_length() {
        // With trade_alpha 1 the expected imbalance is the last trade: 1 for a buy.
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert_eq!(
            trigger_indices(&mut rule, &sided(&[true; 12])),
            vec![3, 7, 11]
        );
    }

    #[test]
    fn alternating_flow_never_reaches_the_expected_imbalance() {
        // The expected imbalance per trade is +-1, so a bar needs |theta| >= 4,
        // which alternating trades never reach.
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        let buys: Vec<bool> = (0..100).map(|i| i % 2 == 0).collect();
        assert!(trigger_indices(&mut rule, &sided(&buys)).is_empty());
    }

    #[test]
    fn volume_imbalance_weighs_by_size() {
        // With E[T] = 2 and trade_alpha 0.5, after each trade:
        // E[b * x] = 10, 0, 7.5, 11.25 and E[|x|] = 10, 10, 12.5, 13.75,
        // thresholds 20, 1 (the floor of 5% of 10, times 2), 15, 22.5,
        // imbalances 10, 0, 15 (closes), 15.
        let trades = [
            trade(0, 100.0, 10.0),
            trade(1, 100.0, -10.0),
            trade(2, 100.0, 15.0),
            trade(3, 100.0, 15.0),
        ];
        let mut rule = ImbalanceRule::new(FlowSignal::Volume(By::Quote), params(2.0, 0.5));
        assert_eq!(trigger_indices(&mut rule, &trades), vec![2]);
    }

    #[test]
    fn trades_without_size_do_not_trigger() {
        let trades: Vec<Trade> = (0..10).map(|ts| trade(ts, 100.0, 0.0)).collect();
        let mut rule = ImbalanceRule::new(FlowSignal::Volume(By::Quote), params(2.0, 0.5));
        assert!(trigger_indices(&mut rule, &trades).is_empty());
    }

    #[test]
    fn one_sided_random_flow_gives_bars_of_about_the_expected_length() {
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(100.0, 0.01));
        let triggers = trigger_indices(&mut rule, &random_flow(200_000, 0.75, 7));
        let mean = mean_bar_length(&triggers);
        assert!((70.0..=130.0).contains(&mean), "mean bar length {mean}");
    }

    #[test]
    fn balanced_random_flow_is_bounded_by_the_floor() {
        // The floor of 5% puts the threshold at 20 or more, which a random walk
        // reaches after about 20^2 = 400 trades.
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(400.0, 0.001));
        let triggers = trigger_indices(&mut rule, &random_flow(400_000, 0.5, 11));
        let mean = mean_bar_length(&triggers);
        assert!((200.0..=800.0).contains(&mean), "mean bar length {mean}");
    }

    #[test]
    fn learned_bar_length_stays_within_its_bounds() {
        let p = FlowBarParams::new(100.0)
            .and_then(|p| p.with_bar_alpha(0.1, 50.0, 200.0))
            .expect("valid parameters");
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, p);
        let flow = random_flow(200_000, 0.5, 3);
        for t in &flow {
            AggregationRule::<OhlcCandle, Trade>::should_trigger(
                &mut rule,
                t,
                &OhlcCandle::default(),
            );
            assert!((50.0..=200.0).contains(&rule.expected_ticks()));
        }
    }

    #[test]
    fn reset_starts_a_new_bar() {
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        let trades = sided(&[true; 6]);
        assert!(trigger_indices(&mut rule, &trades[..3]).is_empty());
        AggregationRule::<OhlcCandle, Trade>::reset(&mut rule);
        assert!(trigger_indices(&mut rule, &trades[3..]).is_empty());
    }

    #[test]
    fn threshold_rule_sets_the_expected_bar_length() {
        let mut rule = ImbalanceRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert_eq!(ThresholdRule::threshold(&rule), 4.0);
        rule.set_threshold(2.0).expect("valid");
        assert_eq!(rule.expected_ticks(), 2.0);
        assert!(rule.set_threshold(0.5).is_err());
        assert!(rule.set_threshold(f64::NAN).is_err());
        assert_eq!(
            trigger_indices(&mut rule, &sided(&[true; 6])),
            vec![1, 3, 5]
        );
    }
}
