use crate::{Trade, Candle, AggTimeStreaming};

// agg_time aggregates trades by timestamp and returns a vector of candles
// threshold in nano-seconds
pub fn agg_time(trades: &Vec<Trade>, candle_period: i64) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();

    let mut agg_time_streaming = AggTimeStreaming::new(candle_period);
    for t in trades {
        let new_candle: bool = agg_time_streaming.update(t);
        if new_candle  {
            out.push(agg_time_streaming.last().clone());
        }
    }
    return out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_agg_time() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let candles = agg_time(&trades, common::H1);

        for i in 0..candles.len() {
            common::test_candle(&candles[i]);
        }
    }
}
