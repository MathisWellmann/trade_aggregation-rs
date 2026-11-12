//! Statistics of the log returns between consecutive trades.
//!
//! The first return of a candle is the one from the last trade of the previous candle,
//! so that no return is lost at a candle boundary and the sum over candles equals the
//! statistic over the whole series.

use crate::{CandleComponent, CandleComponentUpdate, TakerTrade};

/// The log return of `price` from the previous trade price, `None` for the first trade.
#[inline(always)]
fn log_return(previous: Option<f64>, price: f64) -> Option<f64> {
    debug_assert!(price > 0.0, "prices must be positive");
    previous.map(|previous| (price / previous).ln())
}

/// The realized variance: the sum of the squared log returns between consecutive trades.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealizedVariance {
    sum: f64,
    last_price: Option<f64>,
}

impl CandleComponent for RealizedVariance {
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

impl<T: TakerTrade> CandleComponentUpdate<T> for RealizedVariance {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        let price = trade.price();
        if let Some(r) = log_return(self.last_price, price) {
            self.sum += r * r;
        }
        self.last_price = Some(price);
    }
}

/// The bipower variation, `pi / 2 * sum |r_i| * |r_(i-1)|` over consecutive log returns
/// (Barndorff-Nielsen and Shephard). Unlike the realized variance it is robust to jumps,
/// so the difference of the two estimates the jumps.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BipowerVariation {
    sum: f64,
    last_price: Option<f64>,
    last_abs_return: Option<f64>,
}

impl CandleComponent for BipowerVariation {
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

impl<T: TakerTrade> CandleComponentUpdate<T> for BipowerVariation {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        let price = trade.price();
        if let Some(r) = log_return(self.last_price, price) {
            if let Some(previous) = self.last_abs_return {
                self.sum += r.abs() * previous;
            }
            self.last_abs_return = Some(r.abs());
        }
        self.last_price = Some(price);
    }
}

/// The realized quarticity, `n / 3 * sum r^4` over the `n` log returns of the candle,
/// which estimates the variance of the realized variance.
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RealizedQuarticity {
    sum: f64,
    returns: u64,
    last_price: Option<f64>,
}

impl CandleComponent for RealizedQuarticity {
    type Value = f64;

    #[inline(always)]
    fn value(&self) -> f64 {
        self.returns as f64 / 3.0 * self.sum
    }

    /// Keeps the last price, for the first return of the next candle.
    #[inline(always)]
    fn reset(&mut self) {
        self.sum = 0.0;
        self.returns = 0;
    }
}

impl<T: TakerTrade> CandleComponentUpdate<T> for RealizedQuarticity {
    #[inline(always)]
    fn update(&mut self, trade: &T) {
        let price = trade.price();
        if let Some(r) = log_return(self.last_price, price) {
            self.sum += r.powi(4);
            self.returns += 1;
        }
        self.last_price = Some(price);
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
        let mut m = RealizedVariance::default();
        update_all(&mut m, &trades_at_prices(&PRICES));
        let expected: f64 = returns(&PRICES).iter().map(|r| r * r).sum();
        assert!((m.value() - expected).abs() < 1e-15);
    }

    #[test]
    fn the_first_return_of_a_candle_starts_at_the_previous_close() {
        let trades = trades_at_prices(&PRICES);
        let mut whole = RealizedVariance::default();
        update_all(&mut whole, &trades);
        let mut split = RealizedVariance::default();
        update_all(&mut split, &trades[..2]);
        let first = split.value();
        split.reset();
        assert_eq!(split.value(), 0.0);
        update_all(&mut split, &trades[2..]);
        assert!((first + split.value() - whole.value()).abs() < 1e-15);

        let mut empty = RealizedVariance::default();
        update_all(&mut empty, &trades[..1]);
        assert_eq!(empty.value(), 0.0, "one trade has no return");
    }

    #[test]
    fn bipower_variation_multiplies_neighbouring_returns() {
        let mut m = BipowerVariation::default();
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
        let mut rv = RealizedVariance::default();
        let mut bv = BipowerVariation::default();
        update_all(&mut rv, &trades);
        update_all(&mut bv, &trades);
        assert!(rv.value() > 0.009);
        assert!(bv.value() < 0.0005);
    }

    #[test]
    fn realized_quarticity_scales_by_the_number_of_returns() {
        let mut m = RealizedQuarticity::default();
        update_all(&mut m, &trades_at_prices(&PRICES));
        let r = returns(&PRICES);
        let expected = r.len() as f64 / 3.0 * r.iter().map(|r| r.powi(4)).sum::<f64>();
        assert!((m.value() - expected).abs() < 1e-18);
        m.reset();
        assert_eq!(m.value(), 0.0);
    }
}
