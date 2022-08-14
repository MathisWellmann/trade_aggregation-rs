use crate::welford_online::WelfordOnline;
use crate::{Aggregator, Candle, Trade};

#[derive(Debug, Clone)]
/// Struct used for aggregating trades by time in an online (streaming) manner
pub struct TimeAggregator {
    period: i64,
    init: bool,
    init_timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    buy_volume: f64,
    wp: f64,
    price_sum: f64,
    num_trades: i32,
    num_buys: i32,
    welford_prices: WelfordOnline,
    welford_sizes: WelfordOnline,
}

impl TimeAggregator {
    /// Create a new streaming aggregator using timestamps to aggregate candles
    /// with a given candle_period, measured in seconds
    pub fn new(candle_period: i64) -> Self {
        TimeAggregator {
            period: candle_period,
            init: true,
            init_timestamp: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            volume: 0.0,
            buy_volume: 0.0,
            wp: 0.0,
            price_sum: 0.0,
            num_trades: 0,
            num_buys: 0,
            welford_prices: WelfordOnline::new(),
            welford_sizes: WelfordOnline::new(),
        }
    }
}

impl Aggregator for TimeAggregator {
    fn update(&mut self, trade: &Trade) -> Option<Candle> {
        if self.init {
            self.init = false;
            self.init_timestamp = trade.timestamp;
            self.open = trade.price;
            self.high = trade.price;
            self.low = trade.price;
            self.volume = 0.0;
            self.buy_volume = 0.0;
            self.num_trades = 0;
            self.num_buys = 0;
            self.wp = 0.0;
            self.price_sum = 0.0;
            self.welford_sizes.reset();
            self.welford_prices.reset();
        }

        if trade.price > self.high {
            self.high = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
        self.volume += trade.size.abs();
        self.num_trades += 1;
        if trade.size > 0.0 {
            self.num_buys += 1;
            self.buy_volume += trade.size.abs();
        }

        self.wp += trade.price * trade.size.abs();
        self.price_sum += trade.price;

        self.welford_prices.add(trade.price);
        self.welford_sizes.add(trade.size);

        if trade.timestamp - self.init_timestamp > self.period * 1000 {
            let mut elapsed_s: f64 = (trade.timestamp - self.init_timestamp) as f64 / 1000.0;
            if elapsed_s < 1.0 {
                // cap elapsed_s to avoid time_velocity being infinite
                elapsed_s = 1.0;
            }
            let time_velocity: f64 = 1.0 / elapsed_s;

            // create new candle
            let c = Candle {
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.low,
                close: trade.price,
                volume: self.volume,
                directional_trade_ratio: self.num_buys as f64 / self.num_trades as f64,
                directional_volume_ratio: self.buy_volume / self.volume,
                num_trades: self.num_trades,
                arithmetic_mean_price: self.price_sum / self.num_trades as f64,
                weighted_price: self.wp / self.volume,
                std_dev_prices: self.welford_prices.std_dev(),
                std_dev_sizes: self.welford_sizes.std_dev(),
                time_velocity,
            };
            self.init = true;
            return Some(c);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::tests::test_candle;
    use crate::{load_trades_from_csv, H1};

    #[test]
    fn test_agg_time_streaming() {
        let trades = load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv").unwrap();
        let mut agg_time = TimeAggregator::new(H1);

        for i in 0..trades.len() {
            match agg_time.update(&trades[i]) {
                Some(candle) => test_candle(&candle),
                None => {}
            }
        }
    }
}
