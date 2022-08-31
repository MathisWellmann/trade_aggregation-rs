use crate::{AggregationRule, By, ModularCandle, TakerTrade};

/// Creates candles every n units of volume traded
pub struct VolumeRule {
    // If true, the cumulative volume needs to be reset
    init: bool,

    // See docs on By enum for details
    by: By,

    // cumulative volume
    cum_vol: f64,

    // The theshold volume the candle needs to have before finishing it
    threshold_vol: f64,
}

impl VolumeRule {
    /// Create a new instance with the given volume threshold
    pub fn new(threshold_vol: f64, by: By) -> Self {
        Self {
            init: true,
            by,
            cum_vol: 0.0,
            threshold_vol,
        }
    }
}

impl<C, T> AggregationRule<C, T> for VolumeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.init {
            self.cum_vol = 0.0;
            self.init = false;
        }
        self.cum_vol += match self.by {
            By::Quote => trade.size().abs(),
            By::Base => trade.size().abs() / trade.price(),
        };

        let should_trigger = self.cum_vol > self.threshold_vol;
        if should_trigger {
            self.init = true;
        }

        should_trigger
    }
}
