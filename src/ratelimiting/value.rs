use concurrent_map::ConcurrentMap;

use super::{Limiter, LimiterUpdate};

#[derive(Clone)]
pub struct ValueLimiter {
    small_cost: u64,
    large_cost: u64,
    points_per_30: u64,
    max_points: u64,
}

impl ValueLimiter {
    pub fn new(small_cost: u64, large_cost: u64, points_per_30: u64, max_points: u64) -> Self {
        Self {
            small_cost,
            large_cost,
            points_per_30,
            max_points,
        }
    }
}

impl Limiter for ValueLimiter {
    fn check_allowed(
        &self,
        current_time: u64,
        data_prefix: &str,
        data: &ConcurrentMap<String, u64>,
        identity: &str,
    ) -> Result<LimiterUpdate, String> {
        Ok(LimiterUpdate {
            data: identity.to_string(),
            value: 0,
        })
    }
}
