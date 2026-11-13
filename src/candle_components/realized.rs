//! Statistics of the log returns between consecutive prices of a [`PriceSource`]:
//! the trade price by default, or the mid price or the microprice of the quotes.
//!
//! The first return of a candle is the one from the last price of the previous candle,
//! so that no return is lost at a candle boundary and the sum over candles equals the
//! statistic over the whole series.

use crate::{
    price_source::LogReturns, CandleComponent, CandleComponentUpdate, PriceSource, TradePrice,
};

/// The realized variance: the sum of the squared log returns between consecutive prices.
///
/// The type parameter selects the price, e.g. `RealizedVariance<MidPrice>` for the mid price
/// of the quotes, which does not add the bid-ask bounce of the trades to the variance.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealizedVariance<P = TradePrice> {
    sum: f64,
    returns: LogReturns<P>,
}

impl<P> CandleComponent for RealizedVariance<P> {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.sum
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for RealizedVariance<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        if let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) {
            self.sum += r * r;
        }
        debug_assert!(self.sum >= 0.0);
    }
}

/// The bipower variation, `pi / 2 * sum |r_i| * |r_(i-1)|` over consecutive log returns
/// (Barndorff-Nielsen and Shephard). Unlike the realized variance it is robust to jumps,
/// so the difference of the two estimates the jumps.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BipowerVariation<P = TradePrice> {
    sum: f64,
    returns: LogReturns<P>,
    last_abs_return: Option<f64>,
}

impl<P> CandleComponent for BipowerVariation<P> {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        std::f64::consts::FRAC_PI_2 * self.sum
    }

    /// Keeps the last price and return, for the first products of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sum = 0.0;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for BipowerVariation<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) else {
            return;
        };
        if let Some(previous) = self.last_abs_return.replace(r.abs()) {
            self.sum += r.abs() * previous;
        }
        debug_assert!(self.sum >= 0.0);
    }
}

/// The realized quarticity, `n / 3 * sum r^4` over the `n` log returns of the candle,
/// which estimates the variance of the realized variance.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealizedQuarticity<P = TradePrice> {
    sum: f64,
    count: u64,
    returns: LogReturns<P>,
}

impl<P> CandleComponent for RealizedQuarticity<P> {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.count as f64 / 3.0 * self.sum
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sum = 0.0;
        self.count = 0;
    }
}

impl<E, P: PriceSource<E>> CandleComponentUpdate<E> for RealizedQuarticity<P> {
    #[inline(always)]
    fn update(&mut self, event: &E) {
        if let Some(r) = self.returns.observe(event).and_then(|step| step.log_return) {
            self.sum += r.powi(4);
            self.count += 1;
        }
        debug_assert!(self.sum >= 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{aggregation_rules::test_utils::trades_at_prices, Trade};

    fn returns(prices: &[f64]) -> Vec<f64> {
        prices.windows(2).map(|w| (w[1] / w[0]).ln()).collect()
    }

    fn update_all<C: CandleComponentUpdate<Trade>>(c: &mut C, trades: &[Trade]) {
        for t in trades {
            c.update(t);
        }
    }

    const PRICES: [f64; 5] = [100.0, 101.0, 99.0, 99.0, 102.0];

    #[test]
    fn realized_variance_sums_squared_log_returns() {
        let mut m = RealizedVariance::<TradePrice>::default();
        update_all(&mut m, &trades_at_prices(&PRICES));
        let expected: f64 = returns(&PRICES).iter().map(|r| r * r).sum();
        assert!((m.value() - expected).abs() < 1e-15);
    }

    #[test]
    fn the_first_return_of_a_candle_starts_at_the_previous_close() {
        let trades = trades_at_prices(&PRICES);
        let mut whole = RealizedVariance::<TradePrice>::default();
        update_all(&mut whole, &trades);
        let mut split = RealizedVariance::<TradePrice>::default();
        update_all(&mut split, &trades[..2]);
        let first = split.value();
        split.reset();
        assert_eq!(split.value(), 0.0);
        update_all(&mut split, &trades[2..]);
        assert!((first + split.value() - whole.value()).abs() < 1e-15);

        let mut empty = RealizedVariance::<TradePrice>::default();
        update_all(&mut empty, &trades[..1]);
        assert_eq!(empty.value(), 0.0, "one trade has no return");
    }

    #[test]
    fn bipower_variation_multiplies_neighbouring_returns() {
        let mut m = BipowerVariation::<TradePrice>::default();
        update_all(&mut m, &trades_at_prices(&PRICES));
        let r = returns(&PRICES);
        let expected: f64 = r.windows(2).map(|w| w[0].abs() * w[1].abs()).sum::<f64>()
            * std::f64::consts::FRAC_PI_2;
        assert!((m.value() - expected).abs() < 1e-15);
    }

    #[test]
    fn bipower_variation_ignores_a_single_jump() {
        // A jump between small moves adds about its square to the realized variance,
        // but only its products with the small neighbours to the bipower variation.
        let prices = [100.0, 100.1, 100.0, 110.0, 110.1, 110.0];
        let trades = trades_at_prices(&prices);
        let mut rv = RealizedVariance::<TradePrice>::default();
        let mut bv = BipowerVariation::<TradePrice>::default();
        update_all(&mut rv, &trades);
        update_all(&mut bv, &trades);
        assert!(rv.value() > 0.009);
        assert!(bv.value() < 0.0005);
    }

    #[test]
    fn realized_quarticity_scales_by_the_number_of_returns() {
        let mut m = RealizedQuarticity::<TradePrice>::default();
        update_all(&mut m, &trades_at_prices(&PRICES));
        let r = returns(&PRICES);
        let expected = r.len() as f64 / 3.0 * r.iter().map(|r| r.powi(4)).sum::<f64>();
        assert!((m.value() - expected).abs() < 1e-18);
        m.reset();
        assert_eq!(m.value(), 0.0);
    }

    #[test]
    fn the_mid_price_ignores_the_bid_ask_bounce() {
        use crate::{MarketEvent, MidPrice, Quote};
        let quote = MarketEvent::Quote(Quote {
            timestamp: 0,
            bid_price: 99.0,
            bid_size: 1.0,
            ask_price: 101.0,
            ask_size: 1.0,
        });
        let at = |price: f64| {
            MarketEvent::Trade(Trade {
                timestamp: 0,
                price,
                size: 1.0,
            })
        };
        let events = [quote, at(99.0), at(101.0), at(99.0), quote, at(101.0)];
        let mut trades = RealizedVariance::<TradePrice>::default();
        let mut mid = RealizedVariance::<MidPrice>::default();
        for e in &events {
            trades.update(e);
            mid.update(e);
        }
        assert!(trades.value() > 0.001);
        assert_eq!(mid.value(), 0.0);
    }
}
