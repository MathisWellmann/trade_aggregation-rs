use crate::{AggregationRule, ModularCandle, Trade};

/// Creates candles every n units of volume traded
pub struct VolumeRule {
    // If true, the cumulative volume needs to be reset
    init: bool,

    // cumulative volume
    cum_vol: f64,

    // The theshold volume the candle needs to have before finishing it
    threshold_vol: f64,
}

impl VolumeRule {
    /// Create a new instance with the given volume threshold
    pub fn new(threshold_vol: f64) -> Self {
        Self {
            init: true,
            cum_vol: 0.0,
            threshold_vol,
        }
    }
}

impl<C> AggregationRule<C> for VolumeRule
where
    C: ModularCandle,
{
    fn should_trigger(&mut self, trade: &Trade, _candle: &C) -> bool {
        if self.init {
            self.cum_vol = 0.0;
            self.init = false;
        }
        self.cum_vol += trade.size.abs();

        let should_trigger = self.cum_vol > self.threshold_vol;
        if should_trigger {
            self.init = true;
        }

        should_trigger
    }
}
