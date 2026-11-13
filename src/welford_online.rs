/// Algorithm for online estimation of standard deviation
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WelfordOnline {
    count: u32,
    mean: f64,
    s: f64,
}

impl WelfordOnline {
    pub fn new() -> Self {
        WelfordOnline {
            count: 0,
            mean: 0.0,
            s: 0.0,
        }
    }

    /// The sample variance. `None` with fewer than 2 observations: a single observation
    /// has no spread to measure, and dividing by `count - 1` would divide by 0.
    pub fn variance(&self) -> Option<f64> {
        (self.count > 1).then(|| self.s / (self.count as f64 - 1.0))
    }

    /// The sample standard deviation. `None` with fewer than 2 observations.
    pub fn std_dev(&self) -> Option<f64> {
        self.variance().map(f64::sqrt)
    }

    /// The running mean of the observations added so far, 0.0 without an observation yet
    /// (meaningless then; callers should gate on [`Self::std_dev`] or their own count first).
    pub fn mean(&self) -> f64 {
        self.mean
    }

    // reset to defaults
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.s = 0.0;
    }

    // add updates the statistics
    pub fn add(&mut self, val: f64) {
        self.count += 1;
        let old_mean = self.mean;
        self.mean += (val - old_mean) / self.count as f64;
        self.s += (val - old_mean) * (val - self.mean);
    }
}

#[cfg(test)]
mod tests {
    use round::round;

    use super::*;

    const VALS: [f64; 4] = [1.0, 2.0, 1.0, 2.0];

    #[test]
    fn welford_online() {
        let mut welford = WelfordOnline::new();

        for v in &VALS {
            welford.add(*v);
        }
        assert_eq!(round(welford.std_dev().expect("4 observations"), 4), 0.5774);
    }

    #[test]
    fn fewer_than_two_observations_have_no_variance() {
        let mut welford = WelfordOnline::new();
        assert_eq!(welford.variance(), None);
        assert_eq!(welford.std_dev(), None);
        welford.add(1.0);
        assert_eq!(welford.variance(), None, "a single value has no spread");
        welford.add(2.0);
        assert!(welford.variance().is_some());
    }
}
