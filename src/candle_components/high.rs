use crate::{AsTrade, CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the high price.
/// A candle without trades is flat at the close of the previous candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct High {
    high: f64,
    /// Whether the candle has no trade yet.
    empty: bool,
    /// The latest trade price, which an empty candle shows.
    last: f64,
}

impl Default for High {
    fn default() -> Self {
        Self {
            high: f64::MIN,
            empty: true,
            last: f64::MIN,
        }
    }
}

impl CandleComponent for High {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.high
    }

    /// An empty candle shows the previous close until its first trade.
    #[inline(always)]
    fn reset(&mut self) {
        self.high = self.last;
        self.empty = true;
    }
}

impl<E: AsTrade> CandleComponentUpdate<E> for High {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(trade) = event.as_trade() else {
            return;
        };
        let price = trade.price();
        if self.empty || price > self.high {
            self.high = price;
        }
        self.empty = false;
        self.last = price;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high() {
        let mut m = High::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 105.0);
    }
}
