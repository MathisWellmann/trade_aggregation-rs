use crate::{Trade, Candle, By, AggVolumeStreaming};

// agg_volume aggregates trades by volume
pub fn agg_volume(trades: &Vec<Trade>, threshold: f64, by: By) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::new();

    let mut agg_vol_streaming = AggVolumeStreaming::new(threshold, by);
    for t in trades {
        let new_candle: bool = agg_vol_streaming.update(t);
        if new_candle {
            out.push(agg_vol_streaming.last().clone());
        }
    }
    return out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common;

    #[test]
    fn test_agg_volume_base() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let agg_volume = agg_volume(&trades, 1000.0, By::Base);

        for i in 0..agg_volume.len() {
            common::test_candle(&agg_volume[i]);
        }
    }

    #[test]
    fn test_agg_volume_quote() {
        let trades = common::load_trades_from_csv("data/Bitmex_XBTUSD_1M.csv");
        let agg_volume = agg_volume(&trades, 1000.0, By::Quote);

        for i in 0..agg_volume.len() {
            common::test_candle(&agg_volume[i]);
        }
    }
}
