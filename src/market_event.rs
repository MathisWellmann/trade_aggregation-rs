//! Events other than trades, and the projections that let trade rules and trade components
//! work on streams that mix trades with other events.

use crate::{TakerTrade, TimestampResolution, Timestamped};

/// The best bid and ask of an order book (level 1), e.g. a quote update of an exchange.
pub trait BestQuote: Timestamped {
    /// The highest price a buyer is willing to pay.
    fn bid_price(&self) -> f64;

    /// The size at the best bid.
    fn bid_size(&self) -> f64;

    /// The lowest price a seller is willing to accept.
    fn ask_price(&self) -> f64;

    /// The size at the best ask.
    fn ask_size(&self) -> f64;

    /// The mid price, halfway between the bid and the ask.
    #[inline(always)]
    fn mid_price(&self) -> f64 {
        (self.bid_price() + self.ask_price()) / 2.0
    }

    /// The weighted average price (WAP): the bid and the ask weighted by the size on the other side,
    /// so it leans towards the side that is more likely to be taken next.
    /// The mid price if both sizes are zero.
    #[inline(always)]
    fn weighted_price(&self) -> f64 {
        let sizes = self.bid_size() + self.ask_size();
        if sizes <= 0.0 {
            return self.mid_price();
        }
        (self.bid_price() * self.ask_size() + self.ask_price() * self.bid_size()) / sizes
    }
}

/// An event of a market: a trade or a quote, to aggregate both streams of a symbol in time order
/// into one candle.
///
/// Trade rules and trade components act on the trades and ignore the quotes ([`AsTrade`]),
/// quote components act on the quotes ([`AsQuote`]), and time rules act on every event.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MarketEvent<T, Q> {
    /// A taker trade.
    Trade(T),
    /// A quote update.
    Quote(Q),
}

/// The resolution of both event types, which must be the same.
/// Evaluated at compile time, so a mismatch fails the build.
const fn same_resolution(
    trade: TimestampResolution,
    quote: TimestampResolution,
) -> TimestampResolution {
    assert!(
        trade.units_per_millisecond() == quote.units_per_millisecond(),
        "the trades and quotes of a MarketEvent must have the same timestamp resolution"
    );
    trade
}

impl<T: Timestamped, Q: Timestamped> Timestamped for MarketEvent<T, Q> {
    const TIMESTAMP_RESOLUTION: TimestampResolution =
        same_resolution(T::TIMESTAMP_RESOLUTION, Q::TIMESTAMP_RESOLUTION);

    #[inline(always)]
    fn timestamp(&self) -> i64 {
        match self {
            MarketEvent::Trade(t) => t.timestamp(),
            MarketEvent::Quote(q) => q.timestamp(),
        }
    }
}

/// An event that may be a trade: a [`TakerTrade`] itself, or the trades of a [`MarketEvent`].
pub trait AsTrade: Timestamped {
    /// The type of the trade.
    type Trade: TakerTrade;

    /// The trade, `None` if the event is something else.
    fn as_trade(&self) -> Option<&Self::Trade>;
}

impl<T: TakerTrade> AsTrade for T {
    type Trade = T;

    #[inline(always)]
    fn as_trade(&self) -> Option<&T> {
        Some(self)
    }
}

impl<T: TakerTrade, Q: Timestamped> AsTrade for MarketEvent<T, Q> {
    type Trade = T;

    #[inline(always)]
    fn as_trade(&self) -> Option<&T> {
        match self {
            MarketEvent::Trade(t) => Some(t),
            MarketEvent::Quote(_) => None,
        }
    }
}

/// An event that may be a quote: a [`BestQuote`] itself, or the quotes of a [`MarketEvent`].
pub trait AsQuote: Timestamped {
    /// The type of the quote.
    type Quote: BestQuote;

    /// The quote, `None` if the event is something else.
    fn as_quote(&self) -> Option<&Self::Quote>;
}

impl<Q: BestQuote> AsQuote for Q {
    type Quote = Q;

    #[inline(always)]
    fn as_quote(&self) -> Option<&Q> {
        Some(self)
    }
}

impl<T: Timestamped, Q: BestQuote> AsQuote for MarketEvent<T, Q> {
    type Quote = Q;

    #[inline(always)]
    fn as_quote(&self) -> Option<&Q> {
        match self {
            MarketEvent::Trade(_) => None,
            MarketEvent::Quote(q) => Some(q),
        }
    }
}

/// A quote with a millisecond timestamp, the counterpart of [`Trade`](crate::Trade).
#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quote {
    /// Timestamp in milliseconds
    pub timestamp: i64,
    /// The best bid price
    pub bid_price: f64,
    /// The size at the best bid
    pub bid_size: f64,
    /// The best ask price
    pub ask_price: f64,
    /// The size at the best ask
    pub ask_size: f64,
}

impl Timestamped for Quote {
    const TIMESTAMP_RESOLUTION: TimestampResolution = TimestampResolution::Millisecond;

    #[inline(always)]
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
}

impl BestQuote for Quote {
    #[inline(always)]
    fn bid_price(&self) -> f64 {
        self.bid_price
    }

    #[inline(always)]
    fn bid_size(&self) -> f64 {
        self.bid_size
    }

    #[inline(always)]
    fn ask_price(&self) -> f64 {
        self.ask_price
    }

    #[inline(always)]
    fn ask_size(&self) -> f64 {
        self.ask_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Trade;

    type Event = MarketEvent<Trade, Quote>;

    fn quote(timestamp: i64) -> Quote {
        Quote {
            timestamp,
            bid_price: 99.0,
            bid_size: 3.0,
            ask_price: 101.0,
            ask_size: 1.0,
        }
    }

    #[test]
    fn market_events_project_to_their_variant() {
        let trade = Event::Trade(Trade {
            timestamp: 5,
            price: 100.0,
            size: 1.0,
        });
        let q = Event::Quote(quote(7));
        assert_eq!(trade.timestamp(), 5);
        assert_eq!(q.timestamp(), 7);
        assert!(trade.as_trade().is_some());
        assert!(trade.as_quote().is_none());
        assert!(q.as_trade().is_none());
        assert_eq!(q.as_quote(), Some(&quote(7)));
        assert_eq!(
            Event::TIMESTAMP_RESOLUTION,
            TimestampResolution::Millisecond
        );
    }

    #[test]
    fn plain_trades_and_quotes_project_to_themselves() {
        let t = Trade {
            timestamp: 1,
            price: 2.0,
            size: 3.0,
        };
        assert_eq!(t.as_trade(), Some(&t));
        assert_eq!(quote(1).as_quote(), Some(&quote(1)));
    }

    #[test]
    fn the_weighted_price_leans_towards_the_thin_side() {
        let q = quote(0);
        assert_eq!(q.mid_price(), 100.0);
        // Three on the bid, one on the ask: the ask is more likely to be taken.
        assert_eq!(q.weighted_price(), (99.0 * 1.0 + 101.0 * 3.0) / 4.0);
        let empty = Quote {
            bid_size: 0.0,
            ask_size: 0.0,
            ..q
        };
        assert_eq!(empty.weighted_price(), 100.0);
    }
}
