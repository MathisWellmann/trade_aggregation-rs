use crate::welford_online::WelfordOnline;
use crate::{Aggregator, By, Candle, Trade};

#[derive(Debug, Clone)]
/// Used for aggregating trades based on volume
pub struct VolumeAggregator {
    vol_threshold: f64,
    by: By,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    buy_volume: f64,
    wp: f64,
    price_sum: f64,
    init: bool,
    num_trades: i32,
    num_buys: i32,
    welford_prices: WelfordOnline,
    welford_sizes: WelfordOnline,
    init_time: i64,
}

impl VolumeAggregator {
    /// Create a new streaming volume aggregator
    ///
    /// # Arguments:
    /// vol_threshold: create a new candle after this total volume has been reached
    /// by: determines how to interpret the trade size, either as denoted in QUOTE or in BASE
    ///
    pub fn new(vol_threshold: f64, by: By) -> Self {
        return VolumeAggregator {
            vol_threshold,
            by,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            volume: 0.0,
            buy_volume: 0.0,
            wp: 0.0,
            price_sum: 0.0,
            init: true,
            num_trades: 0,
            num_buys: 0,
            welford_prices: WelfordOnline::new(),
            welford_sizes: WelfordOnline::new(),
            init_time: 0,
        };
    }

    /// Return the currently set volume threshold
    #[inline(always)]
    pub fn volume_threshold(&self) -> f64 {
        self.vol_threshold
    }
}

impl Aggregator for VolumeAggregator {
    fn update(&mut self, trade: &Trade) -> Option<Candle> {
        if self.init {
            self.init = false;
            self.open = trade.price;
            self.high = trade.price;
            self.low = trade.price;
            self.volume = 0.0;
            self.buy_volume = 0.0;
            self.wp = 0.0;
            self.price_sum = 0.0;
            self.num_trades = 0;
            self.num_buys = 0;
            self.welford_sizes.reset();
            self.welford_prices.reset();
            self.init_time = trade.timestamp;
        }
        if trade.price > self.high {
            self.high = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }

        if trade.size > 0.0 {
            self.num_buys += 1;
        }

        match self.by {
            By::Base => {
                self.volume += trade.size.abs() / trade.price;
                if trade.size > 0.0 {
                    self.buy_volume += trade.size.abs() / trade.price;
                }
                self.wp += trade.size.abs();
            }
            By::Quote => {
                self.volume += trade.size.abs();
                if trade.size > 0.0 {
                    self.buy_volume += trade.size.abs();
                }
                self.wp += trade.price * trade.size.abs();
            }
        }

        self.num_trades += 1;
        self.price_sum += trade.price;

        self.welford_sizes.add(trade.size);
        self.welford_prices.add(trade.price);

        if self.volume > self.vol_threshold {
            let mut elapsed_s: f64 = (trade.timestamp - self.init_time) as f64 / 1000.0;
            if elapsed_s < 1.0 {
                // cap elapsed_s to avoid time_velocity being infinite
                elapsed_s = 1.0;
            }
            let time_velocity = 1.0 / elapsed_s;

            // create new candle
            let c = Candle {
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.low,
                close: trade.price,
                volume: self.volume,
                weighted_price: self.wp / self.volume,
                arithmetic_mean_price: self.price_sum / self.num_trades as f64,
                num_trades: self.num_trades,
                directional_trade_ratio: self.num_buys as f64 / self.num_trades as f64,
                directional_volume_ratio: self.buy_volume / self.volume,
                std_dev_prices: self.welford_prices.std_dev(),
                std_dev_sizes: self.welford_sizes.std_dev(),
                time_velocity,
            };
            self.init = true;
            return Some(c);
        }
        return None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_trades_from_csv;
    use crate::tests::test_candle;

    #[test]
    fn test_agg_volume_streaming_base() {
        let mut agg_volume = VolumeAggregator::new(1000.0, By::Base);

        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        for i in 0..trades.len() {
            match agg_volume.update(&trades[i]) {
                Some(candle) => test_candle(&candle),
                None => {}
            }
        }
    }
}
