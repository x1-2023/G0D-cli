use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay = self.base_delay.mul_f64(self.multiplier.powi(attempt as i32));
        let capped = delay.min(self.max_delay);
        if self.jitter {
            let jitter_ms = (rand_jitter() * capped.as_millis() as f64 * 0.3) as u64;
            capped + Duration::from_millis(jitter_ms)
        } else {
            capped
        }
    }
}

fn rand_jitter() -> f64 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64);
    (hasher.finish() as f64) / (u64::MAX as f64)
}
