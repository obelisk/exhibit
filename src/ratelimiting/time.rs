use concurrent_map::ConcurrentMap;

use crate::IdentifiedUserMessage;

use super::{Limiter, LimiterUpdate};

#[derive(Clone)]
pub struct TimeLimiter {
    interval: u64,
}

impl TimeLimiter {
    pub fn new(interval: u64) -> Self {
        Self { interval }
    }
}

impl Limiter for TimeLimiter {
    fn check_allowed(
        &self,
        current_time: u64,
        data_prefix: &str,
        data: &ConcurrentMap<String, u64>,
        message: &IdentifiedUserMessage,
    ) -> Result<LimiterUpdate, String> {
        let identity = &message.identity;
        // If they've never sent a message then it's effectively 0
        let previous_send = data
            .get(&format!("{data_prefix}-{identity}"))
            .map(|x| x.to_owned())
            .unwrap_or(0);

        if previous_send > current_time {
            error!(
                "{} last sent an emoji in the future. Not allowing this new send",
                identity
            );

            return Err("Try again shortly.".to_string());
        }

        // Check if this message should be blocked
        if (current_time - previous_send) < self.interval {
            return Err(format!(
                "Try again in {} seconds",
                current_time - previous_send
            ));
        }

        Ok(LimiterUpdate {
            data: identity.to_string(),
            value: current_time,
        })
    }
}
