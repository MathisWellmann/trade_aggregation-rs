use crate::common::{Trade, Candle, BASE, ASSET};
use crate::welford_online;


#[derive(Debug, Clone)]
pub struct AggVolumeStreaming {
    pub last_candle: Candle,
    vol_threshold: f64,
    by: usize,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    buy_volume: f64,
    wp: f64,
    init: bool,
    num_trades: i32,
    num_buys: i32,
    welford_prices: welford_online::WelfordOnline,
    welford_sizes: welford_online::WelfordOnline,
}

impl AggVolumeStreaming {
    pub fn new(vol_threshold: f64, by: usize) -> AggVolumeStreaming {
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
                trade_direction_ratio: 0.0,
                volume_direction_ratio: 0.0,
                std_dev_prices: 0.0,
                std_dev_sizes: 0.0,
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
            welford_prices: welford_online::new(),
            welford_sizes: welford_online::new(),
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
        }
        if trade.price > self.high {
            self.high = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
        if self.by == ASSET {
            self.volume += trade.size.abs() / trade.price;
            if trade.size > 0.0 {
                self.buy_volume += trade.size.abs() / trade.price;
            }
            self.wp += trade.size.abs();
        } else if self.by == BASE {
            self.volume += trade.size.abs();
            if trade.size > 0.0 {
                self.buy_volume += trade.size.abs();
            }
            self.wp += trade.price * trade.size.abs();
        }
        self.num_trades += 1;

        self.welford_sizes.add(trade.size);
        self.welford_prices.add(trade.price);

        if self.volume > self.vol_threshold {
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
                trade_direction_ratio: self.num_buys as f64 / self.num_trades as f64,
                volume_direction_ratio: self.buy_volume / self.volume,
                std_dev_prices: self.welford_prices.std_dev(),
                std_dev_sizes: self.welford_sizes.std_dev(),
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
    fn test_agg_volume_streaming_base() {
        let mut agg_volume = AggVolumeStreaming::new(1000.0, BASE);

        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        for i in 0..trades.len() {
            let new_candle = agg_volume.update(&trades[i]);
            if new_candle {
                common::test_candle(&agg_volume.last_candle);
            }
        }
    }

    #[test]
    fn test_agg_volume_streaming_asset() {
        let mut agg_volume = AggVolumeStreaming::new(1000.0, ASSET);

        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        for i in 0..trades.len() {
            let new_candle = agg_volume.update(&trades[i]);
            if new_candle {
                common::test_candle(&agg_volume.last_candle);
            }
        }
    }
}
