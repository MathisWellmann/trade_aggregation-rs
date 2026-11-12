use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// This 'CandleComponent' keeps track of the low price.
/// A candle without trades is flat at the close of the previous candle.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Low {
    low: f64,
    /// Whether the candle has no trade yet.
    empty: bool,
    /// The latest trade price, which an empty candle shows.
    last: f64,
}

impl Default for Low {
    fn default() -> Self {
        Self {
            low: f64::MAX,
            empty: true,
            last: f64::MAX,
        }
    }
}

impl CandleComponent for Low {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.low
    }

    /// An empty candle shows the previous close until its first trade.
    #[inline(always)]
    fn reset(&mut self) {
        self.low = self.last;
        self.empty = true;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for Low {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        let price = trade.price();
        if self.empty || price < self.low {
            self.low = price;
        }
        self.empty = false;
        self.last = price;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low() {
        let mut m = Low::default();
        for t in &crate::candle_components::tests::TRADES {
            m.update(t);
        }
        assert_eq!(m.value(), 100.0);
    }
}
