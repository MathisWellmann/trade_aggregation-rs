use crate::common::{Trade, Candle};

#[derive(Debug)]
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
}

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
            trade_direction_ratio: 0.0,
            volume_direction_ratio: 0.0,
            num_trades: 0,
            weighted_price: 0.0,
        }
    }
}

impl AggTimeStreaming {
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

        if trade.timestamp - self.init_timestamp > self.period * 1000 {
            // create new candle
            let c = Candle{
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.low,
                close: trade.price,
                volume: self.volume,
                volume_direction_ratio: self.buy_volume / self.volume,
                num_trades: self.num_trades,
                trade_direction_ratio: self.num_buys as f64 / self.num_trades as f64,
                weighted_price: self.wp / self.volume,
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
        let mut agg_time = new(common::H1);

        for i in 0..trades.len() {
            let new_candle = agg_time.update(&trades[i]);
            if new_candle {
                common::test_candle(agg_time.last());
            }
        }
    }
}
