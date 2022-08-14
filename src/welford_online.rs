#[derive(Debug, Clone)]
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

    // variance returns the variance
    pub fn variance(&self) -> f64 {
        if self.count > 1 {
            return self.s / (self.count as f64 - 1.0);
        }

        0.0
    }

    // std_dev returns the standard deviation
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
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
    use super::*;
    use round::round;

    const VALS: [f64; 4] = [1.0, 2.0, 1.0, 2.0];

    #[test]
    fn welford_online() {
        let mut welford = WelfordOnline::new();

        for v in &VALS {
            welford.add(*v);
        }
        assert_eq!(round(welford.std_dev(), 4), 0.5774);
    }
}
