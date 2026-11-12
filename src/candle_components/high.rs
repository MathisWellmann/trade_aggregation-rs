use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

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

impl CandleComponent<f64> for High {
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

impl<T: TakerTrade> CandleComponentUpdate<T> for High {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
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
