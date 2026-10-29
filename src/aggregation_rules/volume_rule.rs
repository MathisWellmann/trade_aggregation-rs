use crate::{AggregationRule, By, Error, ModularCandle, Result, TakerTrade, ThresholdRule};

/// Creates candles every n units of volume traded.
/// If the last trade needed to complete a bucket is for a size greater than required,
/// the excess size is given to the next bucket
#[derive(Debug, Clone)]
pub struct VolumeRule {
    /// See docs on By enum for details
    by: By,

    /// cumulative volume
    cum_vol: f64,

    /// The theshold volume the candle needs to have before finishing it
    threshold_vol: f64,
}

impl VolumeRule {
    /// Create a new instance with the given volume threshold
    pub fn new(threshold_vol: f64, by: By) -> Result<Self> {
        if threshold_vol <= 0.0 {
            return Err(Error::InvalidParam);
        }
        Ok(Self {
            by,
            cum_vol: 0.0,
            threshold_vol,
        })
    }
}

impl<C, T> AggregationRule<C, T> for VolumeRule
where
    C: ModularCandle<T>,
    T: TakerTrade,
{
    fn should_trigger(&mut self, trade: &T, _candle: &C) -> bool {
        if self.cum_vol >= self.threshold_vol {
            // If the last trade needed to complete a bucket is for a size greater than required,
            // the excess size is given to the next bucket
            self.cum_vol -= self.threshold_vol;
            debug_assert!(self.cum_vol >= 0.0);
        }
        self.cum_vol += match self.by {
            By::Quote => trade.size().abs(),
            By::Base => trade.size().abs() / trade.price(),
        };

        self.cum_vol >= self.threshold_vol
    }

    fn reset(&mut self) {
        self.cum_vol = 0.0;
    }
}

impl ThresholdRule for VolumeRule {
    fn threshold(&self) -> f64 {
        self.threshold_vol
    }

    fn set_threshold(&mut self, threshold: f64) -> Result<()> {
        if !threshold.is_finite() || threshold <= 0.0 {
            return Err(Error::InvalidParam);
        }
        // A finished bucket is only emptied on the next trade:
        // empty it with the old threshold, so the excess carries over unchanged.
        if self.cum_vol >= self.threshold_vol {
            self.cum_vol -= self.threshold_vol;
        }
        self.threshold_vol = threshold;
        Ok(())
    }
}
