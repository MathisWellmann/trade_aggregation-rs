use crate::{Trade, Candle, By};
use crate::welford_online::WelfordOnline;


#[derive(Debug, Clone)]
pub struct AggVolumeStreaming {
    pub last_candle: Candle,
    vol_threshold: f64,
    by: By,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    buy_volume: f64,
    wp: f64,
    init: bool,
    num_trades: i32,
    num_buys: i32,
    welford_prices: WelfordOnline,
    welford_sizes: WelfordOnline,
    bid: f64,
    ask: f64,
    spread_sum: f64,
    init_time: i64,
}

impl AggVolumeStreaming {
    pub fn new(vol_threshold: f64, by: By) -> AggVolumeStreaming {
        return AggVolumeStreaming {
            vol_threshold,
            by,
            last_candle: Candle{
                timestamp: 0,
                open: 0.0,
                high: 0.0,
                low: 0.0,
                close: 0.0,
                volume: 0.0,
                weighted_price: 0.0,
                num_trades: 0,
                directional_trade_ratio: 0.0,
                directional_volume_ratio: 0.0,
                std_dev_prices: 0.0,
                std_dev_sizes: 0.0,
                last_spread: 0.0,
                avg_spread: 0.0,
                directional_trade_entropy: 0.0,
                directional_volume_entropy: 0.0,
                time_velocity: 0.0,
            },
            open: 0.0,
            high: 0.0,
            low: 0.0,
            volume: 0.0,
            buy_volume: 0.0,
            wp: 0.0,
            init: true,
            num_trades: 0,
            num_buys: 0,
            welford_prices: WelfordOnline::new(),
            welford_sizes: WelfordOnline::new(),
            bid: 0.0,
            ask: 0.0,
            spread_sum: 0.0,
            init_time: 0,
        }
    }

    // update observes a trade and updates the aggregated candle
    // return true if new candle has been created
    pub fn update(&mut self, trade: &Trade) -> bool {
        if self.init {
            self.init = false;
            self.open = trade.price;
            self.high = trade.price;
            self.low = trade.price;
            self.volume = 0.0;
            self.buy_volume = 0.0;
            self.wp = 0.0;
            self.num_trades = 0;
            self.num_buys = 0;
            self.welford_sizes.reset();
            self.welford_prices.reset();
            self.bid = trade.price;
            self.ask = trade.price;
            self.init_time = trade.timestamp;
        }
        if trade.price > self.high {
            self.high = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
        
        if trade.size > 0.0 {
            self.ask = trade.price;
            self.num_buys += 1;
        } else {
            self.bid = trade.price;
        }
        self.spread_sum += self.ask - self.bid;

        match self.by {
            By::Base => {
                self.volume += trade.size.abs() / trade.price;
                if trade.size > 0.0 {
                    self.buy_volume += trade.size.abs() / trade.price;
                }
                self.wp += trade.size.abs();
            },
            By::Quote => {
                self.volume += trade.size.abs();
                if trade.size > 0.0 {
                    self.buy_volume += trade.size.abs();
                }
                self.wp += trade.price * trade.size.abs();
            }
        }

        self.num_trades += 1;

        self.welford_sizes.add(trade.size);
        self.welford_prices.add(trade.price);

        if self.volume > self.vol_threshold {
            let pb: f64 = self.num_buys as f64 / self.num_trades as f64;  // probability of buy direction
            let ps: f64 = 1.0 - pb;  // probability of sell direction
            let mut directional_trade_entropy: f64 = pb * pb.log2() + ps * ps.log2();
            if directional_trade_entropy.is_nan() {
                directional_trade_entropy = 0.0;
            }

            let pb: f64 = self.buy_volume / self.volume;
            let ps: f64 = 1.0 - pb;
            let mut directional_volume_entropy: f64 = pb * pb.log2() + ps * ps.log2();
            if directional_volume_entropy.is_nan() {
                directional_volume_entropy = 0.0;
            }

            let elapsed_m: f64 = (trade.timestamp - self.init_time) as f64 / 60_000.0;
            let time_velocity = 1.0 / elapsed_m;

            // create new candle
            let c = Candle{
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.low,
                close: trade.price,
                volume: self.volume,
                weighted_price: self.wp / self.volume,
                num_trades: self.num_trades,
                directional_trade_ratio: self.num_buys as f64 / self.num_trades as f64,
                directional_volume_ratio: self.buy_volume / self.volume,
                std_dev_prices: self.welford_prices.std_dev(),
                std_dev_sizes: self.welford_sizes.std_dev(),
                last_spread: self.ask - self.bid,
                avg_spread: self.spread_sum / self.num_trades as f64,
                directional_trade_entropy,
                directional_volume_entropy ,
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

    // sets the volume threshold.
    // caution is adviced as changing it in the middle of candle creation can have unexpected effects
    // it is adviced to only set it after a new candle has been created
    pub fn set_vol_threshold(&mut self, vol_threshold: f64) {
        self.vol_threshold  = vol_threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_agg_volume_streaming_base() {
        let mut agg_volume = AggVolumeStreaming::new(1000.0, By::Base);

        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        for i in 0..trades.len() {
            let new_candle = agg_volume.update(&trades[i]);
            if new_candle {
                common::test_candle(&agg_volume.last_candle);
            }
        }
    }
}
