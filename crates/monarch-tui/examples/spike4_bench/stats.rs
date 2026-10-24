//! Latency samples and their percentiles.

use std::time::Duration;

/// Collected latencies for one scenario.
#[derive(Debug, Default)]
pub struct Stats {
    samples: Vec<Duration>,
}

impl Stats {
    pub fn push(&mut self, d: Duration) {
        self.samples.push(d);
    }

    /// The `p`-th percentile (0–100), nearest rank.
    pub fn percentile(&self, p: f64) -> Duration {
        let mut sorted = self.samples.clone();
        sorted.sort_unstable();
        if sorted.is_empty() {
            return Duration::ZERO;
        }
        let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
        sorted[rank.clamp(1, sorted.len()) - 1]
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let mut s = Stats::default();
        for ms in 1..=100 {
            s.push(Duration::from_millis(ms));
        }
        assert_eq!(s.percentile(50.0), Duration::from_millis(50));
        assert_eq!(s.percentile(99.0), Duration::from_millis(99));
        assert_eq!(s.percentile(100.0), Duration::from_millis(100));
        assert_eq!(Stats::default().percentile(99.0), Duration::ZERO);
    }
}
