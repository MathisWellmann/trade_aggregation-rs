use super::directional_change_rule::{next_trend, Trend};
use crate::{
    price_source::checked_price, AggregationRule, Error, PriceSource, Result, ThresholdRule,
    TradePrice,
};

/// The direction of the trend after an [`IntrinsicEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TrendDirection {
    /// The price rises.
    Up,
    /// The price falls.
    Down,
}

/// An event of intrinsic time, see [`IntrinsicEventRule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum IntrinsicEvent {
    /// The price reversed `threshold_fraction` from the extreme of the trend:
    /// a new trend starts in the given direction.
    DirectionalChange(TrendDirection),
    /// The price moved another `threshold_fraction` beyond the last intrinsic event,
    /// in the direction of the trend.
    Overshoot(TrendDirection),
}

impl IntrinsicEvent {
    /// The direction of the trend after the event.
    pub fn direction(self) -> TrendDirection {
        match self {
            IntrinsicEvent::DirectionalChange(direction) | IntrinsicEvent::Overshoot(direction) => {
                direction
            }
        }
    }

    /// Whether the event is a directional change rather than an overshoot.
    pub fn is_directional_change(self) -> bool {
        matches!(self, IntrinsicEvent::DirectionalChange(_))
    }
}

/// An intrinsic event and the price at which it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct EventAt {
    event: IntrinsicEvent,
    price: f64,
}

/// Closes a candle at every event of the "intrinsic time" of Guillaume et al. (1997) and
/// Glattfelder, Dupuis and Olsen (2011): at each directional change and at each overshoot.
///
/// A directional change (DC) is confirmed as in the
/// [`DirectionalChangeRule`](crate::DirectionalChangeRule): the price reverses
/// `threshold_fraction` from the extreme of the current trend (before the first one, from the
/// extremes since the first price). An overshoot (OS) event marks each further move of
/// `threshold_fraction` in the direction of the trend, measured from the price of the last
/// intrinsic event: in an uptrend at a price of at least `last * (1 + threshold_fraction)`,
/// in a downtrend at a price of at most `last * (1 - threshold_fraction)`.
/// The candles thus cut the price path into moves of about `threshold_fraction`, and the long
/// overshoots that a [`DirectionalChangeRule`](crate::DirectionalChangeRule) keeps in one candle
/// are split up. A price that moves several thresholds at once (a gap) is one event,
/// and the next overshoot is measured from its price, so a gap does not close a burst of
/// candles on the events after it.
///
/// [`Self::last_event`] tells which event closed the candle, to label it. The share of
/// directional changes among all intrinsic events is a stable statistic of the regime,
/// about `1 - 1/e = 0.63` (arXiv 2511.14408): on a random walk the overshoot beyond a directional
/// change is about exponential with a mean of `threshold_fraction`, so it reaches each further
/// threshold with a probability of about `1/e`. A lower share means trends that run on,
/// a higher share a market that reverts.
///
/// The state follows the price path rather than the candle, so [`reset`](AggregationRule::reset)
/// keeps it.
///
/// The prices are those of the trades by default. Trades alternate between the bid and the ask
/// ("bid-ask bounce", Roll 1984), so a threshold near the spread confirms an event on every bounce.
/// Follow the mid price or the microprice of the quotes instead with [`Self::with_price_source`].
///
/// To calibrate the threshold with [`Adaptive`](crate::Adaptive), use
/// [`Scaling::Diffusive`](crate::Scaling::Diffusive): a random walk takes a time proportional to
/// `threshold_fraction^2` to move by `threshold_fraction`.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IntrinsicEventRule<P = TradePrice> {
    threshold_fraction: f64,
    source: P,
    trend: Trend,
    /// `None` before the first directional change.
    last: Option<EventAt>,
}

impl IntrinsicEventRule {
    /// Create a new instance that follows the trade prices.
    ///
    /// # Arguments:
    /// `threshold_fraction`: The reversal that confirms a directional change and the move
    ///     that makes an overshoot, e.g. 0.01 for 1%.
    ///
    /// # Errors:
    /// [`Error::InvalidParam`] if `threshold_fraction` is not in `(0, 1)`.
    pub fn new(threshold_fraction: f64) -> Result<Self> {
        let mut rule = Self {
            threshold_fraction: 0.5,
            source: TradePrice,
            trend: Trend::Start,
            last: None,
        };
        rule.set_threshold(threshold_fraction)?;
        Ok(rule)
    }
}

impl<P> IntrinsicEventRule<P> {
    /// Follow the prices of `source` instead, e.g. [`MidPrice`](crate::MidPrice)
    /// or [`Microprice`](crate::Microprice), with the same threshold.
    /// The rule starts over: the first price of `source` is the start of the path.
    pub fn with_price_source<Q>(self, source: Q) -> IntrinsicEventRule<Q> {
        IntrinsicEventRule {
            threshold_fraction: self.threshold_fraction,
            source,
            trend: Trend::Start,
            last: None,
        }
    }

    /// The event of the last trigger, `None` before the first directional change.
    /// Read it right after a trigger, e.g. through
    /// [`GenericAggregator::rule`](crate::GenericAggregator::rule), to label the candle it closed.
    pub fn last_event(&self) -> Option<IntrinsicEvent> {
        self.last.map(|last| last.event)
    }
}

/// The intrinsic event at `price`, given the trend after it, whether `price` confirmed
/// a directional change, and the price of the last intrinsic event.
fn intrinsic_event(
    trend: Trend,
    confirmed: bool,
    last_price: Option<f64>,
    price: f64,
    threshold: f64,
) -> Option<IntrinsicEvent> {
    use IntrinsicEvent::{DirectionalChange, Overshoot};
    let last = || last_price.expect("a trend starts with a directional change event");
    match trend {
        Trend::Start | Trend::Unknown { .. } => {
            assert!(!confirmed, "a directional change starts a trend");
            None
        }
        Trend::Up { .. } if confirmed => Some(DirectionalChange(TrendDirection::Up)),
        Trend::Down { .. } if confirmed => Some(DirectionalChange(TrendDirection::Down)),
        Trend::Up { .. } => {
            (price >= last() * (1.0 + threshold)).then_some(Overshoot(TrendDirection::Up))
        }
        Trend::Down { .. } => {
            (price <= last() * (1.0 - threshold)).then_some(Overshoot(TrendDirection::Down))
        }
    }
}

impl<E, P: PriceSource<E>> AggregationRule<E> for IntrinsicEventRule<P> {
    fn should_trigger(&mut self, event: &E) -> bool {
        let Some(price) = checked_price(&self.source, event) else {
            return false;
        };
        let (trend, confirmed) = next_trend(self.trend, price, self.threshold_fraction);
        let last_price = self.last.map(|last| last.price);
        let event = intrinsic_event(trend, confirmed, last_price, price, self.threshold_fraction);
        self.trend = trend;
        if let Some(event) = event {
            self.last = Some(EventAt { event, price });
        }
        // The extreme of a trend is at or beyond the last event, which started or extended it.
        match (self.trend, self.last) {
            (Trend::Up { extreme }, Some(last)) => assert!(extreme >= last.price),
            (Trend::Down { extreme }, Some(last)) => assert!(extreme <= last.price),
            (Trend::Up { .. } | Trend::Down { .. }, None) => {
                unreachable!("a trend starts with a directional change event")
            }
            (Trend::Start | Trend::Unknown { .. }, last) => assert!(last.is_none()),
        }
        event.is_some()
    }

    /// Keeps the state: it follows the price path rather than the candle.
    fn reset(&mut self) {}
}

impl<P> ThresholdRule for IntrinsicEventRule<P> {
    fn threshold(&self) -> f64 {
        self.threshold_fraction
    }

    /// A threshold of 1 or more could never confirm a downward change.
    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !(threshold > 0.0 && threshold < 1.0) {
            return Err(Error::InvalidParam);
        }
        self.threshold_fraction = threshold;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregation_rules::test_utils::{
            quote_event, trade, trade_event, trades_at_prices, trigger_indices,
        },
        DirectionalChangeRule, MidPrice, Trade,
    };

    const UP: TrendDirection = TrendDirection::Up;
    const DOWN: TrendDirection = TrendDirection::Down;

    fn rule(threshold: f64) -> IntrinsicEventRule {
        IntrinsicEventRule::new(threshold).expect("valid threshold")
    }

    /// The indices of the events on which `rule` triggers, and the intrinsic event of each.
    fn intrinsic_events<E, P: PriceSource<E>>(
        rule: &mut IntrinsicEventRule<P>,
        events: &[E],
    ) -> Vec<(usize, IntrinsicEvent)> {
        let mut out = Vec::new();
        for (i, e) in events.iter().enumerate() {
            if rule.should_trigger(e) {
                out.push((i, rule.last_event().expect("a trigger has an event")));
            }
        }
        out
    }

    /// A DC up, two overshoots up, a DC down, an overshoot down and a DC up, for 10%.
    const PATH: [f64; 12] = [
        100.0, 105.0, // Unknown: high 105, low 100
        111.0, // 11% above the low of 100: DC up, the next overshoot at 111 * 1.1 = 122.1
        115.0, 123.0, // OS up, the next one at 135.3
        120.0, 135.4, // OS up, the next one at 148.94
        130.0, 121.0, // 10.6% below the high of 135.4: DC down, the next overshoot at 108.9
        112.0, 108.0, // OS down, the next one at 97.2
        118.9, // 10.1% above the low of 108: DC up
    ];

    #[test]
    fn new_rejects_invalid_thresholds() {
        assert!(IntrinsicEventRule::new(0.0).is_err());
        assert!(IntrinsicEventRule::new(1.0).is_err());
        assert!(IntrinsicEventRule::new(-0.1).is_err());
        assert!(IntrinsicEventRule::new(f64::NAN).is_err());
        assert!(IntrinsicEventRule::new(0.999).is_ok());
    }

    #[test]
    fn triggers_at_directional_changes_and_overshoots() {
        use IntrinsicEvent::{DirectionalChange as Dc, Overshoot as Os};
        let trades = trades_at_prices(&PATH);
        let mut r = rule(0.1);
        assert_eq!(r.last_event(), None);
        assert_eq!(
            intrinsic_events(&mut r, &trades),
            vec![
                (2, Dc(UP)),
                (4, Os(UP)),
                (6, Os(UP)),
                (8, Dc(DOWN)),
                (10, Os(DOWN)),
                (11, Dc(UP)),
            ]
        );
        // The directional changes are those of the directional change rule.
        let mut dc = DirectionalChangeRule::new(0.1).expect("valid threshold");
        assert_eq!(trigger_indices(&mut dc, &trades), vec![2, 8, 11]);
    }

    #[test]
    fn a_gap_is_one_overshoot() {
        use IntrinsicEvent::{DirectionalChange as Dc, Overshoot as Os};
        // 89 is a DC down. 60 crosses three thresholds at once: one overshoot,
        // and the next one is due at 60 * 0.9 = 54, not at 89 * 0.9^2 = 72.1.
        let trades = trades_at_prices(&[100.0, 89.0, 60.0, 55.0, 53.9]);
        assert_eq!(
            intrinsic_events(&mut rule(0.1), &trades),
            vec![(1, Dc(DOWN)), (2, Os(DOWN)), (4, Os(DOWN))]
        );
    }

    #[test]
    fn a_trend_without_reversal_is_a_series_of_overshoots() {
        let prices: Vec<f64> = (0..100).map(|i| 100.0 * 1.01_f64.powi(i)).collect();
        let mut r = rule(0.05);
        let events = intrinsic_events(&mut r, &trades_at_prices(&prices));
        // The DC at 1.01^5 = 5.1%, then an overshoot every 5 steps (1.01^5 = 1.051).
        let indices: Vec<usize> = events.iter().map(|(i, _)| *i).collect();
        assert_eq!(indices, (1..20).map(|i| 5 * i).collect::<Vec<_>>());
        assert!(events[0].1.is_directional_change());
        for (_, event) in &events[1..] {
            assert_eq!(*event, IntrinsicEvent::Overshoot(UP));
            assert_eq!(event.direction(), UP);
        }
    }

    #[test]
    fn reset_keeps_the_state() {
        let trades = trades_at_prices(&PATH);
        let mut r = rule(0.1);
        assert_eq!(trigger_indices(&mut r, &trades[..5]), vec![2, 4]);
        AggregationRule::<Trade>::reset(&mut r);
        assert_eq!(r.last_event(), Some(IntrinsicEvent::Overshoot(UP)));
        // The same events as without the reset: 6, 8, 10 and 11.
        assert_eq!(trigger_indices(&mut r, &trades[5..]), vec![1, 3, 5, 6]);
    }

    #[test]
    fn set_threshold_validates() {
        let mut r = rule(0.1);
        r.set_threshold(0.2).expect("valid threshold");
        assert_eq!(r.threshold(), 0.2);
        assert!(r.set_threshold(1.5).is_err());
        assert!(r.set_threshold(f64::NAN).is_err());
        assert_eq!(r.threshold(), 0.2);
    }

    #[test]
    fn the_mid_price_ignores_the_bid_ask_bounce() {
        use IntrinsicEvent::{DirectionalChange as Dc, Overshoot as Os};
        let events = [
            quote_event(99.0, 1.0, 101.0, 1.0),
            trade_event(99.0),
            trade_event(101.0),
            quote_event(99.0, 1.0, 101.0, 1.0),
            trade_event(99.0),
            trade_event(101.0),
            quote_event(101.0, 1.0, 103.0, 1.0),
            quote_event(102.6, 1.0, 104.6, 1.0),
        ];
        // Trades: 99 starts; every bounce of 2% reverses the trend.
        assert_eq!(
            intrinsic_events(&mut rule(0.015), &events),
            vec![(2, Dc(UP)), (4, Dc(DOWN)), (5, Dc(UP))]
        );
        // Mid: 100 starts; 102 is a DC up and 103.6 is 1.57% above it.
        let mut mid = rule(0.015).with_price_source(MidPrice);
        assert_eq!(
            intrinsic_events(&mut mid, &events),
            vec![(6, Dc(UP)), (7, Os(UP))]
        );
        assert_eq!(mid.threshold(), 0.015);
    }

    #[test]
    fn about_63_percent_of_the_events_of_a_random_walk_are_directional_changes() {
        // Log steps of +-0.01% from a seeded xorshift generator, events at 0.2%.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut log_price = 0.0;
        let mut r = rule(0.002);
        let (mut changes, mut overshoots) = (0_u32, 0_u32);
        for _ in 0..2_000_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            log_price += if state >> 63 == 0 { 1e-4 } else { -1e-4 };
            let price = 100.0 * f64::exp(log_price);
            if r.should_trigger(&trade(0, price, 1.0)) {
                match r.last_event().expect("a trigger has an event") {
                    IntrinsicEvent::DirectionalChange(_) => changes += 1,
                    IntrinsicEvent::Overshoot(_) => overshoots += 1,
                }
            }
        }
        assert!(changes > 1_000, "{changes} directional changes");
        let share = f64::from(changes) / f64::from(changes + overshoots);
        let expected = 1.0 - (-1.0_f64).exp();
        assert!((share - expected).abs() < 0.03, "share {share}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn a_restored_rule_continues_the_path() {
        let trades = trades_at_prices(&PATH);
        let mut r = rule(0.1);
        assert_eq!(trigger_indices(&mut r, &trades[..7]), vec![2, 4, 6]);
        let config = bincode::config::standard();
        let bytes = bincode::serde::encode_to_vec(&r, config).expect("serializable");
        let (mut restored, _): (IntrinsicEventRule, _) =
            bincode::serde::decode_from_slice(&bytes, config).expect("deserializable");
        assert_eq!(restored.last_event(), r.last_event());
        assert_eq!(trigger_indices(&mut restored, &trades[7..]), vec![1, 3, 4]);
    }
}
