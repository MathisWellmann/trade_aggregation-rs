//! Which price a price driven rule or a return component follows:
//! the trade price, the mid price or the microprice of the quotes.
//!
//! Trade prices alternate between the bid and the ask ("bid-ask bounce"), so their returns
//! have a negative lag-1 autocorrelation (Roll 1984) and sums of absolute or squared returns
//! grow with the number of trades even when the market does not move.
//! The mid price and the microprice of the quotes do not bounce, so rules and components
//! that follow them react to real moves only. They need a stream with quotes,
//! e.g. a [`MarketEvent`](crate::MarketEvent) of trades and quotes.

use crate::{AsQuote, AsTrade, BestQuote, TakerTrade};

/// The price that a rule or a component follows, read from each event.
///
/// The type parameter of a rule or component selects the source at compile time
/// (e.g. [`TradePrice`], [`MidPrice`] or [`Microprice`]), so that a quote source
/// on a stream without quotes does not compile. [`PriceKind`] selects it at runtime,
/// e.g. from a configuration, for streams of trades and quotes.
pub trait PriceSource<E> {
    /// The price of `event`, or `None` if the event has no such price,
    /// e.g. a quote for [`TradePrice`], a trade for [`MidPrice`],
    /// or a quote without a valid bid and ask.
    fn price(&self, event: &E) -> Option<f64>;
}

/// Follow the prices of the trades ([`TakerTrade::price`]) and ignore other events.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TradePrice;

impl<E: AsTrade> PriceSource<E> for TradePrice {
    #[inline(always)]
    fn price(&self, event: &E) -> Option<f64> {
        event.as_trade().map(TakerTrade::price)
    }
}

/// Follow the mid price of the quotes ([`BestQuote::mid_price`]) and ignore other events.
/// A quote whose bid or ask is not positive and finite, or whose bid is above its ask, has no price.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MidPrice;

impl<E: AsQuote> PriceSource<E> for MidPrice {
    #[inline(always)]
    fn price(&self, event: &E) -> Option<f64> {
        let quote = event.as_quote()?;
        valid_quote(quote).then(|| quote.mid_price())
    }
}

/// Follow the microprice of the quotes and ignore other events:
/// the bid and the ask weighted by the size on the other side
/// ([`BestQuote::weighted_price`], also called the weighted average price or WAP),
/// so that it leans towards the side that is more likely to be taken next.
///
/// It is the first order approximation of the micro-price of Stoikov (2018),
/// which adjusts the mid price by the expected drift given the imbalance and the spread;
/// the full micro-price needs a model fitted to the market, which is out of the scope of a candle.
/// A quote whose bid or ask is not positive and finite, or whose bid is above its ask, has no price.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Microprice;

impl<E: AsQuote> PriceSource<E> for Microprice {
    #[inline(always)]
    fn price(&self, event: &E) -> Option<f64> {
        let quote = event.as_quote()?;
        valid_quote(quote).then(|| quote.weighted_price())
    }
}

/// The price source chosen at runtime, e.g. from a configuration that lists the rules to run,
/// for streams of trades and quotes such as a [`MarketEvent`](crate::MarketEvent).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PriceKind {
    /// [`TradePrice`]
    #[default]
    Trade,
    /// [`MidPrice`]
    Mid,
    /// [`Microprice`]
    Microprice,
}

impl<E: AsTrade + AsQuote> PriceSource<E> for PriceKind {
    #[inline(always)]
    fn price(&self, event: &E) -> Option<f64> {
        match self {
            PriceKind::Trade => TradePrice.price(event),
            PriceKind::Mid => MidPrice.price(event),
            PriceKind::Microprice => Microprice.price(event),
        }
    }
}

/// Whether a quote has a usable bid and ask: both positive and finite, and not crossed
/// (`bid <= ask`). A feed can send a side with a zero price when that side of the book is empty,
/// and the mid price of such a quote would be half the other side.
#[inline(always)]
pub(crate) fn valid_quote<Q: BestQuote>(quote: &Q) -> bool {
    let (bid, ask) = (quote.bid_price(), quote.ask_price());
    bid.is_finite() && ask.is_finite() && bid > 0.0 && ask > 0.0 && bid <= ask
}

/// A price of a [`PriceSource`] and the log return from the previous one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PriceStep {
    /// The price of the event.
    pub(crate) price: f64,
    /// `ln(price / previous price)`, `None` for the first price.
    pub(crate) log_return: Option<f64>,
}

/// Follows the prices of a [`PriceSource`] and computes the log returns between them.
///
/// It keeps the last price over a candle boundary, so that the first return of a candle
/// is the one from the last price of the previous candle and no return is lost.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct LogReturns<P> {
    source: P,
    last: Option<f64>,
}

impl<P> LogReturns<P> {
    pub(crate) fn new(source: P) -> Self {
        Self { source, last: None }
    }

    /// The price of `event` and its log return, `None` if the event has no price of the source.
    #[inline(always)]
    pub(crate) fn observe<E>(&mut self, event: &E) -> Option<PriceStep>
    where
        P: PriceSource<E>,
    {
        let price = self.source.price(event)?;
        assert!(
            price.is_finite() && price > 0.0,
            "prices must be positive and finite, got {price}"
        );
        let log_return = self.last.replace(price).map(|last| (price / last).ln());
        if let Some(r) = log_return {
            debug_assert!(r.is_finite(), "the log return of positive prices is finite");
        }
        Some(PriceStep { price, log_return })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarketEvent, Quote, Trade};

    type Event = MarketEvent<Trade, Quote>;

    fn trade(price: f64) -> Event {
        MarketEvent::Trade(Trade {
            timestamp: 0,
            price,
            size: 1.0,
        })
    }

    fn quote(bid: f64, bid_size: f64, ask: f64, ask_size: f64) -> Event {
        MarketEvent::Quote(Quote {
            timestamp: 0,
            bid_price: bid,
            bid_size,
            ask_price: ask,
            ask_size,
        })
    }

    #[test]
    fn each_source_reads_its_own_events() {
        let (t, q) = (trade(101.0), quote(100.0, 3.0, 102.0, 1.0));
        assert_eq!(TradePrice.price(&t), Some(101.0));
        assert_eq!(TradePrice.price(&q), None);
        assert_eq!(MidPrice.price(&q), Some(101.0));
        assert_eq!(MidPrice.price(&t), None);
        // Three times more size on the bid: the microprice leans towards the ask.
        assert_eq!(Microprice.price(&q), Some(101.5));
        assert_eq!(Microprice.price(&t), None);
    }

    #[test]
    fn a_plain_trade_has_a_trade_price() {
        let t = Trade {
            timestamp: 0,
            price: 7.0,
            size: -1.0,
        };
        assert_eq!(TradePrice.price(&t), Some(7.0));
    }

    #[test]
    fn the_runtime_kind_matches_the_static_sources() {
        let events = [trade(101.0), quote(100.0, 3.0, 102.0, 1.0)];
        for e in &events {
            assert_eq!(PriceKind::Trade.price(e), TradePrice.price(e));
            assert_eq!(PriceKind::Mid.price(e), MidPrice.price(e));
            assert_eq!(PriceKind::Microprice.price(e), Microprice.price(e));
        }
        assert_eq!(PriceKind::default(), PriceKind::Trade);
    }

    #[test]
    fn invalid_quotes_have_no_price() {
        for q in [
            quote(0.0, 1.0, 102.0, 1.0),
            quote(100.0, 1.0, 0.0, 1.0),
            quote(102.0, 1.0, 100.0, 1.0),
            quote(f64::NAN, 1.0, 100.0, 1.0),
            quote(100.0, 1.0, f64::INFINITY, 1.0),
        ] {
            assert_eq!(MidPrice.price(&q), None);
            assert_eq!(Microprice.price(&q), None);
        }
        // A locked quote is valid.
        assert_eq!(MidPrice.price(&quote(100.0, 1.0, 100.0, 1.0)), Some(100.0));
    }

    #[test]
    fn a_quote_without_sizes_has_the_mid_as_microprice() {
        assert_eq!(
            Microprice.price(&quote(100.0, 0.0, 102.0, 0.0)),
            Some(101.0)
        );
    }

    #[test]
    fn log_returns_start_at_the_second_price_and_skip_other_events() {
        let mut returns = LogReturns::new(MidPrice);
        assert_eq!(returns.observe(&trade(1.0)), None);
        let first = returns
            .observe(&quote(99.0, 1.0, 101.0, 1.0))
            .expect("a quote has a mid price");
        assert_eq!(first.price, 100.0);
        assert_eq!(first.log_return, None);
        assert_eq!(returns.observe(&trade(1.0)), None);
        let second = returns
            .observe(&quote(109.0, 1.0, 111.0, 1.0))
            .expect("a quote has a mid price");
        assert_eq!(second.price, 110.0);
        assert_eq!(second.log_return, Some((110.0_f64 / 100.0).ln()));
    }

    #[test]
    #[should_panic(expected = "prices must be positive")]
    fn a_non_positive_trade_price_panics() {
        let _ = LogReturns::new(TradePrice).observe(&trade(0.0));
    }
}
