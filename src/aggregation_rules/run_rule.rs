use crate::{
    aggregation_rules::flow_bars::{check_ticks, Ewma},
    AggregationRule, FlowBarParams, FlowSignal, Result, TakerTrade, ThresholdRule,
};

/// Creates the tick, volume or dollar run bars of López de Prado
/// (Advances in Financial Machine Learning, ch. 2.3.2.3 and 2.3.2.4).
///
/// A bar of `T` trades tracks the flow of each side, `buys = sum x` over the taker buys and
/// `sells = sum x` over the taker sells, with `x` the contribution of a trade (1 for ticks, the size for volume).
/// It closes at the first trade where
/// `max(buys, sells) >= E[T] * max(E[x * 1_buy], E[x * 1_sell])`:
/// one side took more of the bar than the expected bar length and the expected flow per side predict.
/// Despite the name the trades of a side need not be consecutive; trades of the other side in between
/// do not restart the count. So run bars catch a large order executed in slices,
/// even when other trades are mixed in, which the net flow of an [`ImbalanceRule`](crate::ImbalanceRule)
/// partly cancels.
///
/// The expected flow per side is a moving average over trades and `E[T]` the expected bar length,
/// see [`FlowBarParams`]. Balanced flow gives bars somewhat shorter than `E[T]`, because the larger
/// of the two sides runs ahead of half the flow; a learned `E[T]` therefore drifts down to its lower bound.
/// [`ThresholdRule`] reads and sets `E[T]`, so [`Adaptive`](crate::Adaptive) can calibrate it
/// to a target duration; leave [`FlowBarParams::with_bar_alpha`] unset then.
#[derive(Debug, Clone)]
pub struct RunRule {
    signal: FlowSignal,
    params: FlowBarParams,
    expected_ticks: f64,
    expected_buys: Ewma,
    expected_sells: Ewma,
    buys: f64,
    sells: f64,
    ticks: f64,
}

impl RunRule {
    /// Create a new instance.
    ///
    /// # Arguments:
    /// `signal`: What each trade contributes: 1 (tick run bars) or its size (volume or dollar run bars).
    /// `params`: The expected bar length and how the expectations are learned.
    pub fn new(signal: FlowSignal, params: FlowBarParams) -> Self {
        Self {
            signal,
            params,
            expected_ticks: params.expected_ticks(),
            expected_buys: Ewma::new(params.trade_alpha()),
            expected_sells: Ewma::new(params.trade_alpha()),
            buys: 0.0,
            sells: 0.0,
            ticks: 0.0,
        }
    }

    /// The expected number of trades per bar, `E[T]`.
    pub fn expected_ticks(&self) -> f64 {
        self.expected_ticks
    }
}

impl<C, T: TakerTrade> AggregationRule<C, T> for RunRule {
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        let flow = self.signal.flow(trade);
        let (buy, sell) = if flow.side > 0.0 {
            (flow.magnitude, 0.0)
        } else if flow.side < 0.0 {
            (0.0, flow.magnitude)
        } else {
            (0.0, 0.0)
        };
        self.expected_buys.update(buy);
        self.expected_sells.update(sell);
        self.buys += buy;
        self.sells += sell;
        self.ticks += 1.0;

        let threshold =
            self.expected_ticks * self.expected_buys.get().max(self.expected_sells.get());
        debug_assert!(threshold >= 0.0);
        // Without any flow so far there is nothing to compare against.
        let trigger = threshold > 0.0 && self.buys.max(self.sells) >= threshold;
        if trigger {
            self.expected_ticks = self
                .params
                .next_expected_ticks(self.expected_ticks, self.ticks);
            self.buys = 0.0;
            self.sells = 0.0;
            self.ticks = 0.0;
        }
        debug_assert!(self.expected_ticks >= 1.0);
        trigger
    }

    /// Start a new bar; the expectations stay.
    fn reset(&mut self) {
        self.buys = 0.0;
        self.sells = 0.0;
        self.ticks = 0.0;
    }
}

impl ThresholdRule for RunRule {
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
        Adaptive, By, ImbalanceRule, MillisecondPeriod, Trade,
    };

    /// Trades of size 1 one millisecond apart, alternating buy and sell, starting with a buy.
    fn alternating(n: i64) -> Vec<Trade> {
        (0..n)
            .map(|ts| trade(ts, 100.0, if ts % 2 == 0 { 1.0 } else { -1.0 }))
            .collect()
    }

    fn params(expected_ticks: f64, trade_alpha: f64) -> FlowBarParams {
        FlowBarParams::new(expected_ticks)
            .and_then(|p| p.with_trade_alpha(trade_alpha))
            .expect("valid parameters")
    }

    #[test]
    fn persistent_buying_closes_bars_of_the_expected_length() {
        let trades: Vec<Trade> = (0..12).map(|ts| trade(ts, 100.0, 1.0)).collect();
        let mut rule = RunRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert_eq!(trigger_indices(&mut rule, &trades), vec![3, 7, 11]);
    }

    #[test]
    fn interleaved_trades_do_not_restart_a_run() {
        // With trade_alpha 1 the expected flow of the last trade's side is 1, so a side
        // needs 4 trades in the bar: the buys reach 4 on trade 6, then the sells on trade 13.
        let trades = alternating(16);
        let mut rule = RunRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert_eq!(trigger_indices(&mut rule, &trades), vec![6, 13]);
        // The net flow of the same trades never builds up.
        let mut imbalance = ImbalanceRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert!(trigger_indices(&mut imbalance, &trades).is_empty());
    }

    #[test]
    fn volume_runs_weigh_by_size() {
        // With E[T] = 4 and trade_alpha 0.5, after each trade:
        // E[x * 1_buy] = 0, 0.5, 0.25, 0.625; E[x * 1_sell] = 5, 2.5, 3.75, 1.875;
        // thresholds 20, 10, 15, 7.5; larger side 5, 5, 10, 10 (closes).
        let trades = [
            trade(0, 100.0, -5.0),
            trade(1, 100.0, 1.0),
            trade(2, 100.0, -5.0),
            trade(3, 100.0, 1.0),
        ];
        let mut rule = RunRule::new(FlowSignal::Volume(By::Quote), params(4.0, 0.5));
        assert_eq!(trigger_indices(&mut rule, &trades), vec![3]);
    }

    #[test]
    fn trades_without_size_do_not_trigger() {
        let trades: Vec<Trade> = (0..10).map(|ts| trade(ts, 100.0, 0.0)).collect();
        let mut rule = RunRule::new(FlowSignal::Volume(By::Quote), params(2.0, 0.5));
        assert!(trigger_indices(&mut rule, &trades).is_empty());
    }

    #[test]
    fn balanced_random_flow_gives_bars_somewhat_shorter_than_expected() {
        let mut rule = RunRule::new(FlowSignal::Ticks, params(100.0, 0.01));
        let triggers = trigger_indices(&mut rule, &random_flow(200_000, 0.5, 5));
        let mean =
            (triggers[triggers.len() - 1] - triggers[0]) as f64 / (triggers.len() - 1) as f64;
        assert!((60.0..=105.0).contains(&mean), "mean bar length {mean}");
    }

    #[test]
    fn learned_bar_length_stays_within_its_bounds() {
        let p = FlowBarParams::new(100.0)
            .and_then(|p| p.with_bar_alpha(0.1, 50.0, 200.0))
            .expect("valid parameters");
        let mut rule = RunRule::new(FlowSignal::Ticks, p);
        for t in &random_flow(100_000, 0.5, 9) {
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
        let trades: Vec<Trade> = (0..6).map(|ts| trade(ts, 100.0, 1.0)).collect();
        let mut rule = RunRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert!(trigger_indices(&mut rule, &trades[..3]).is_empty());
        AggregationRule::<OhlcCandle, Trade>::reset(&mut rule);
        assert!(trigger_indices(&mut rule, &trades[3..]).is_empty());
    }

    #[test]
    fn threshold_rule_sets_the_expected_bar_length() {
        let mut rule = RunRule::new(FlowSignal::Ticks, params(4.0, 1.0));
        assert_eq!(ThresholdRule::threshold(&rule), 4.0);
        rule.set_threshold(2.0).expect("valid");
        assert_eq!(rule.expected_ticks(), 2.0);
        assert!(rule.set_threshold(0.0).is_err());
        assert_eq!(rule.expected_ticks(), 2.0);
    }

    #[test]
    fn adaptive_calibrates_the_expected_bar_length_to_a_duration() {
        // One trade per millisecond, alternating sides: a bar of E[T] = k lasts about 2k ms.
        let rule = RunRule::new(FlowSignal::Ticks, params(10.0, 1.0));
        let mut rule = Adaptive::new(rule, MillisecondPeriod::from_non_zero(100), 0.5, 1.0, 1e6)
            .expect("valid parameters");
        let triggers = trigger_indices(&mut rule, &alternating(20_000));
        let threshold = rule.inner().expected_ticks();
        assert!((49.0..=52.0).contains(&threshold), "E[T] {threshold}");
        for pair in triggers[triggers.len() - 10..].windows(2) {
            let length = pair[1] - pair[0];
            assert!((95..=105).contains(&length), "bar of {length} ms");
        }
    }
}
