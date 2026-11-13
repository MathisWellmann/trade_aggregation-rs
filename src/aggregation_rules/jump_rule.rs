use std::f64::consts::FRAC_PI_2;

use crate::{price_source::LogReturns, AggregationRule, Error, PriceSource, Result, TradePrice};

/// Closes a candle at a price jump: a log return with `|r| >= k * sigma`,
/// where `sigma` is a local, jump-robust estimate of the standard deviation of a return.
///
/// `sigma^2` is an exponentially weighted moving average of the bipower terms
/// `pi / 2 * |r_i| * |r_(i-1)|` of consecutive returns (the factor `pi / 2` undoes
/// `E|Z| = sqrt(2 / pi)` of a standard normal `Z`). As in the jump test of Lee and Mykland (2008),
/// which scales each return by the bipower variation of the returns before it, a jump enters
/// a term multiplied by an ordinary return, so it inflates the estimate far less than its square
/// would. Each return is tested against `sigma` from before that return: the estimate never
/// contains the return under test, so there is no lookahead and a jump cannot mask itself.
/// The first `warmup` returns only build the estimate.
///
/// Zero returns are skipped: they neither trigger nor enter the estimate, so `sigma` is the
/// standard deviation per change of the price. Trade prices repeat often (consecutive trades
/// at the same price), and a zero next to a return makes a bipower term of zero, so counting
/// zeros would shrink `sigma` with the share of repeated prices and turn ordinary moves into jumps.
///
/// The prices are those of the trades by default. Trades also bounce between the bid and the ask
/// (Roll 1984), which inflates `sigma` to about the spread and hides every jump of the market
/// smaller than `k` spreads. Prefer the mid price or the microprice of the quotes with
/// [`Self::with_price_source`].
///
/// Jumps are rare by design, so use the rule next to one that closes the regular candles:
/// `AnyRule::new(base, jump_rule)` with [`TriggerTrade::NextCandle`](crate::TriggerTrade::NextCandle)
/// lets the jump open a fresh candle instead of blending it into the current one.
/// The estimate follows the price path rather than the candle, so
/// [`reset`](AggregationRule::reset) keeps it.
/// `k` counts standard deviations, not an activity that accumulates over a candle,
/// so the rule does not implement [`ThresholdRule`](crate::ThresholdRule).
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct JumpRule<P = TradePrice> {
    k: f64,
    alpha: f64,
    warmup: usize,
    returns: LogReturns<P>,
    /// `|r|` of the last non-zero return.
    last_size: Option<f64>,
    /// The moving average of the bipower terms, `None` before the second non-zero return.
    variance: Option<f64>,
    /// The number of non-zero returns so far, saturating.
    seen: usize,
}

impl JumpRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `k`: The size of a jump in standard deviations, e.g. 4 to 6.
    /// `alpha`: The weight of the newest bipower term in the moving average, in `(0, 1]`.
    ///     About `2 / (n + 1)` remembers as much as a window of `n` returns.
    /// `warmup`: The number of non-zero returns that only build the estimate, at least 2
    ///     (the first bipower term needs two returns).
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `k` is not positive and finite, if `alpha` is not in `(0, 1]`,
    /// or if `warmup` is below 2.
    pub fn new(k: f64, alpha: f64, warmup: usize) -> Result<Self> {
        if !k.is_finite() || k <= 0.0 {
            return Err(Error::InvalidParam);
        }
        if !(alpha > 0.0 && alpha <= 1.0) {
            return Err(Error::InvalidParam);
        }
        if warmup < 2 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            k,
            alpha,
            warmup,
            returns: LogReturns::new(TradePrice),
            last_size: None,
            variance: None,
            seen: 0,
        })
    }
}

impl<P> JumpRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same parameters and a new estimate.
    pub fn with_price_source<Q>(self, source: Q) -> JumpRule<Q> {
        JumpRule {
            k: self.k,
            alpha: self.alpha,
            warmup: self.warmup,
            returns: LogReturns::new(source),
            last_size: None,
            variance: None,
            seen: 0,
        }
    }

    /// The current estimate of the standard deviation of a non-zero log return,
    /// including the last return, e.g. to scale the jump or the returns of the next candle.
    /// `None` before the second non-zero return.
    pub fn sigma(&self) -> Option<f64> {
        self.variance.map(f64::sqrt)
    }
}

/// The moving average of the bipower terms after adding `term`.
fn moving_average(average: Option<f64>, term: f64, alpha: f64) -> f64 {
    match average {
        None => term,
        Some(average) => average + alpha * (term - average),
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for JumpRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return false;
        };
        if r == 0.0 {
            return false;
        }
        let size = r.abs();
        debug_assert!(size > 0.0);
        if self.seen >= self.warmup {
            assert!(self.variance.is_some(), "two returns give a bipower term");
        }
        // Test against the estimate from before this return.
        let jump = match self.variance {
            Some(variance) if self.seen >= self.warmup => size >= self.k * variance.sqrt(),
            _ => false,
        };
        if let Some(last_size) = self.last_size.replace(size) {
            let term = FRAC_PI_2 * size * last_size;
            self.variance = Some(moving_average(self.variance, term, self.alpha));
        }
        self.seen = self.seen.saturating_add(1);
        if self.seen >= 2 {
            debug_assert!(self.variance.is_some_and(|v| v >= 0.0));
        }
        jump
    }

    /// Keeps the estimate: it follows the price path rather than the candle.
    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_all_trades,
        aggregation_rules::test_utils::{
            prices_from_returns, quote_event, trade_event, trades_at_prices, trigger_indices,
        },
        plot::OhlcCandle,
        AnyRule, GenericAggregator, Microprice, MidPrice, TickRule, Trade, TriggerTrade,
    };

    fn rule(k: f64, alpha: f64, warmup: usize) -> JumpRule {
        JumpRule::new(k, alpha, warmup).expect("valid params")
    }

    /// Log returns of `size` that alternate in sign, starting upwards.
    fn alternating(n: usize, size: f64) -> Vec<f64> {
        (0..n)
            .map(|i| if i % 2 == 0 { size } else { -size })
            .collect()
    }

    /// Six returns of 0.1%, an up jump of 1%, four returns of 0.1% and a down jump of 1%.
    fn two_jumps() -> Vec<f64> {
        let mut returns = alternating(6, 0.001);
        returns.push(0.01);
        returns.extend(alternating(4, -0.001));
        returns.push(-0.01);
        returns
    }

    #[test]
    fn new_rejects_invalid_params() {
        for k in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(JumpRule::new(k, 0.1, 10).is_err(), "k {k}");
        }
        for alpha in [0.0, -0.1, 1.01, f64::NAN] {
            assert!(JumpRule::new(4.0, alpha, 10).is_err(), "alpha {alpha}");
        }
        for warmup in [0, 1] {
            assert!(JumpRule::new(4.0, 0.1, warmup).is_err(), "warmup {warmup}");
        }
        assert!(JumpRule::new(4.0, 1.0, 2).is_ok());
    }

    #[test]
    fn triggers_on_jumps_in_both_directions() {
        // Before the first jump sigma = sqrt(pi / 2) * 0.1% = 0.125%, so 4 sigma = 0.50% <= 1%.
        // The jump lifts sigma^2 to 2.98e-6 and the next return, whose term holds the jump,
        // to 4.26e-6; three more returns bring it down to 3.53e-6: 4 sigma = 0.75% <= 1%.
        let trades = trades_at_prices(&prices_from_returns(&two_jumps()));
        let mut r = rule(4.0, 0.1, 4);
        assert_eq!(trigger_indices(&mut r, &trades), vec![7, 12]);
        let sigma = r.sigma().expect("warm");
        assert!(sigma > 0.001, "sigma {sigma}");
        assert!(sigma < 0.003, "sigma {sigma}");
    }

    #[test]
    fn the_warmup_only_builds_the_estimate() {
        let trades = trades_at_prices(&prices_from_returns(&[0.001, -0.001, 0.01]));
        assert_eq!(trigger_indices(&mut rule(4.0, 0.1, 2), &trades), vec![3]);
        assert!(trigger_indices(&mut rule(4.0, 0.1, 3), &trades).is_empty());
        let mut r = rule(4.0, 0.1, 2);
        assert!(trigger_indices(&mut r, &trades[..2]).is_empty());
        assert_eq!(r.sigma(), None, "one return gives no bipower term");
    }

    #[test]
    fn a_jump_does_not_mask_itself() {
        // With alpha = 1, sigma is the last bipower term. Tested against the terms before it,
        // the jump of 1% is 8 sigma; with its own term, sigma^2 would be pi / 2 * 1% * 0.1%
        // and 4 sigma = 1.59% would hide it.
        let trades = trades_at_prices(&prices_from_returns(&[0.001, 0.001, 0.01]));
        let mut r = rule(4.0, 1.0, 2);
        assert_eq!(trigger_indices(&mut r, &trades), vec![3]);
        let expected = (FRAC_PI_2 * 0.01 * 0.001_f64).sqrt();
        let sigma = r.sigma().expect("warm");
        assert!((sigma - expected).abs() < 1e-9, "sigma {sigma}");
    }

    #[test]
    fn zero_returns_are_skipped() {
        let mut r = rule(4.0, 0.5, 2);
        let trades = trades_at_prices(&[100.0, 100.1, 100.1, 100.0, 100.0, 100.0, 100.1]);
        assert!(trigger_indices(&mut r, &trades[..4]).is_empty());
        let sigma = r.sigma().expect("two non-zero returns");
        let expected = FRAC_PI_2.sqrt() * (100.1_f64 / 100.0).ln();
        assert!((sigma - expected).abs() < 1e-12, "sigma {sigma}");
        // The repeated prices neither trigger nor shrink sigma towards zero.
        assert!(trigger_indices(&mut r, &trades[4..]).is_empty());
        let after = r.sigma().expect("warm");
        assert!((after - expected).abs() < 1e-12, "sigma {after}");
    }

    #[test]
    fn reset_keeps_the_estimate() {
        let trades = trades_at_prices(&prices_from_returns(&two_jumps()));
        let mut r = rule(4.0, 0.1, 4);
        assert!(trigger_indices(&mut r, &trades[..6]).is_empty());
        let sigma = r.sigma();
        AggregationRule::<Trade>::reset(&mut r);
        assert_eq!(r.sigma(), sigma);
        // The same jumps as without the reset, at trades 7 and 12.
        assert_eq!(trigger_indices(&mut r, &trades[6..]), vec![1, 6]);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_restored_rule_keeps_its_estimate() {
        let trades = trades_at_prices(&prices_from_returns(&two_jumps()));
        let mut r = rule(4.0, 0.1, 4);
        assert!(trigger_indices(&mut r, &trades[..6]).is_empty());
        let config = bincode::config::standard();
        let bytes = bincode::serde::encode_to_vec(&r, config).expect("serializable");
        let (mut restored, _): (JumpRule, _) =
            bincode::serde::decode_from_slice(&bytes, config).expect("deserializable");
        assert_eq!(restored.sigma(), r.sigma());
        assert_eq!(trigger_indices(&mut restored, &trades[6..]), vec![1, 6]);
    }

    #[test]
    fn a_jump_opens_a_fresh_candle() {
        let prices = prices_from_returns(&two_jumps());
        let trades = trades_at_prices(&prices);
        let rule = AnyRule::new(TickRule::new(1_000), rule(4.0, 0.1, 4));
        let mut aggregator = GenericAggregator::<OhlcCandle, _, Trade>::with_trigger_trade(
            rule,
            TriggerTrade::NextCandle,
        );
        let candles: Vec<_> = aggregate_all_trades(&trades, &mut aggregator)
            .into_iter()
            .map(crate::Bar::into_candle)
            .collect();
        assert_eq!(candles.len(), 2);
        // The first candle ends before the jump, the second one opens at the jump.
        assert_eq!(candles[0].close(), prices[6]);
        assert_eq!(candles[1].open(), prices[7]);
        assert_eq!(candles[1].close(), prices[11]);
    }

    #[test]
    fn the_mid_price_shows_the_jump_that_the_bounce_hides() {
        // The mid steps by 0.01% and then jumps by 0.1%; the trades bounce by 0.2%.
        let mids = [
            100.0, 100.01, 100.0, 100.01, 100.0, 100.01, 100.0, 100.01, 100.11,
        ];
        let events: Vec<_> = mids
            .iter()
            .flat_map(|&mid| {
                [
                    quote_event(mid - 0.1, 1.0, mid + 0.1, 1.0),
                    trade_event(mid - 0.1),
                    trade_event(mid + 0.1),
                ]
            })
            .collect();
        // Mid: sigma = sqrt(pi / 2) * 0.01% = 0.0125%, and the jump at the last quote is 8 sigma.
        let mut mid = rule(4.0, 0.1, 4).with_price_source(MidPrice);
        assert_eq!(trigger_indices(&mut mid, &events), vec![24]);
        let mut micro = rule(4.0, 0.1, 4).with_price_source(Microprice);
        assert_eq!(trigger_indices(&mut micro, &events), vec![24]);
        // Trades: sigma = sqrt(pi / 2) * 0.2% = 0.25%, so only a move of 1% would be a jump.
        let mut trades = rule(4.0, 0.1, 4);
        assert!(trigger_indices(&mut trades, &events).is_empty());
        assert!(trades.sigma().expect("warm") > 0.002);
    }
}
