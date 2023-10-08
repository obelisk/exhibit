use concurrent_map::ConcurrentMap;

use crate::IdentifiedUserMessage;

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
        message: &IdentifiedUserMessage,
    ) -> Result<LimiterUpdate, String> {
        let identity = &message.identity;
        // If they've never sent a message then it's whatever their starting balance is
        let balance = data
            .get(&format!("{data_prefix}-{identity}"))
            .map(|x| x.to_owned())
            .unwrap_or(self.max_points);

        if balance <= 0 {
            return Err("Cannot send more emojis. Out of balance!".to_string());
        }

        Ok(LimiterUpdate {
            data: identity.to_string(),
            value: balance - 1,
        })
    }
}
