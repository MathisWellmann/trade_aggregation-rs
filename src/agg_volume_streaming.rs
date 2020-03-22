use crate::common::{Trade, Candle};

pub const ASSET: i8 = 0;
pub const BASE_CURRENCY: i8 = 1;

pub struct AggVolumeStreaming {
    pub last_candle: Candle,
    vol_threshold: f64,
    by: i8,
    open: f64,
    high: f64,
    low: f64,
    volume: f64,
    wp: f64,
    init: bool,
    num_trades: i8,
    trade_direction_ratio: f64,
    volume_direction_ratio: f64,
}


pub fn new(vol_threshold: f64, by: i8) -> AggVolumeStreaming {
    return AggVolumeStreaming {
        vol_threshold: vol_threshold,
        by: by,
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
        },
        open: 0.0,
        high: 0.0,
        low: 0.0,
        volume: 0.0,
        wp: 0.0,
        init: true,
        num_trades: 0,
        trade_direction_ratio: 0.0,
        volume_direction_ratio: 0.0,
    }
}

impl AggVolumeStreaming {
    // update observes a trade and updates the aggregated candle
    // return true if new candle has been created
    pub fn update(&mut self, trade: &Trade) -> bool {
        if self.init {
            self.init = false;
            self.open = trade.price;
            self.high = trade.price;
            self.low = trade.price;
        }
        if trade.price > self.high {
            self.high = trade.price;
        }
        if trade.price < self.low {
            self.low = trade.price;
        }
        if self.by == ASSET {
            self.volume += trade.size.abs() / trade.price;
            self.wp += trade.size.abs();
        } else if self.by == BASE_CURRENCY {
            self.volume += trade.size.abs();
            self.wp += trade.price * trade.size.abs();
        }

        if self.volume > self.vol_threshold {
            // create new candle
            let c = Candle{
                timestamp: trade.timestamp,
                open: self.open,
                high: self.high,
                low: self.high,
                close: trade.price,
                volume: self.volume,
                weighted_price: self.wp / self.volume,
                num_trades: self.num_trades,
                trade_direction_ratio: self.trade_direction_ratio,
                volume_direction_ratio: self.volume_direction_ratio,
            };
            self.last_candle = c;
            self.init = true;
            return true
        }
        return false
    }
}
