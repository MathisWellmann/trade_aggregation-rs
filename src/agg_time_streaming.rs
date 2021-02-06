use crate::common::{Trade, Candle};
use crate::welford_online::WelfordOnline;


#[derive(Debug, Clone)]
pub struct AggTimeStreaming {
    period: i64,
    init: bool,
    init_timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    buy_volume: f64,
    wp: f64,
    num_trades: i32,
    num_buys: i32,
    last_candle: Candle,
    welford_prices: WelfordOnline,
    welford_sizes: WelfordOnline,
    bid: f64,
    ask: f64,
    spread_sum: f64,
}

impl AggTimeStreaming {
    pub fn new(candle_period: i64) -> AggTimeStreaming {
        return AggTimeStreaming{
            period: candle_period,
            init: true,
            init_timestamp: 0,
            open: 0.0,
            high: 0.0,
            low: 0.0,
            volume: 0.0,
            buy_volume: 0.0,
            wp: 0.0,
            num_trades: 0,
            num_buys: 0,
            last_candle: Candle{
                timestamp: 0,
                open: 0.0,
                high: 0.0,
                low: 0.0,
                close: 0.0,
                volume: 0.0,
                directional_trade_ratio: 0.0,
                directional_volume_ratio: 0.0,
                num_trades: 0,
                weighted_price: 0.0,
                std_dev_prices: 0.0,
                std_dev_sizes: 0.0,
                last_spread: 0.0,
                avg_spread: 0.0,
                time_velocity: 1.0,
            },
            welford_prices: WelfordOnline::new(),
            welford_sizes: WelfordOnline::new(),
            bid: 0.0,
            ask: 0.0,
            spread_sum: 0.0,
        }
    }

    pub fn update(&mut self, trade: &Trade) -> bool {
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
            self.welford_sizes.reset();
            self.welford_prices.reset();
            self.bid = trade.price;
            self.ask = trade.price;
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
            self.ask = trade.price;
        } else {
            self.bid = trade.price;
        }
        self.spread_sum += self.ask - self.bid;

        self.wp += trade.price * trade.size.abs();

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
            let c = Candle{
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.low,
                close: trade.price,
                volume: self.volume,
                directional_trade_ratio: self.num_buys as f64 / self.num_trades as f64,
                directional_volume_ratio: self.buy_volume / self.volume,
                num_trades: self.num_trades,
                weighted_price: self.wp / self.volume,
                std_dev_prices: self.welford_prices.std_dev(),
                std_dev_sizes: self.welford_sizes.std_dev(),
                last_spread: self.ask - self.bid,
                avg_spread: self.spread_sum / self.num_trades as f64,
                time_velocity,
            };
            self.last_candle = c;
            self.init = true;
            return true
        }
        return false
    }

    pub fn last(&self) -> &Candle {
        return &self.last_candle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_agg_time_streaming() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let mut agg_time = AggTimeStreaming::new(common::H1);

        for i in 0..trades.len() {
            let new_candle = agg_time.update(&trades[i]);
            if new_candle {
                common::test_candle(agg_time.last());
            }
        }
    }
}
