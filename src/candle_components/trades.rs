use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// A `CandleComponent` that gathers all observed trades and returns them.
/// Be careful, the `value` method clones the inner vector,
/// due to the trait definition and lifetime restrictions.
/// So call sparingly.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trades<T>
where
    T: TakerTrade,
{
    trades: Vec<T>,
}

impl<T> CandleComponent<Vec<T>> for Trades<T>
where
    T: TakerTrade + Clone,
{
    #[inline(always)]
    fn value(&self) -> Vec<T> {
        // NOTE: due to the trait definition and borrowing lifetimes,
        // the return type cannot be a reference,
        // Would be better if we borrow here.
        self.trades.clone()
    }

    #[inline(always)]
    fn reset(&mut self) {
        self.trades.clear();
    }
}

impl<T> CandleComponentUpdate<T> for Trades<T>
where
    T: TakerTrade + Clone,
{
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        self.trades.push(trade.clone());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn trades() {
        let mut comp = Trades::default();
        for t in &crate::candle_components::tests::TRADES {
            comp.update(t);
        }
        assert_eq!(comp.value(), crate::candle_components::tests::TRADES);
    }
}
