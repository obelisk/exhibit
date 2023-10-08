use concurrent_map::ConcurrentMap;

use crate::UserMessage;

use super::{Limiter, LimiterUpdate};

#[derive(Clone)]
pub struct TimeLimiter {
    interval: u64,
}

impl Limiter for TimeLimiter {
    fn check_allowed(
        &self,
        data_prefix: &str,
        data: &ConcurrentMap<String, u64>,
        identity: &str,
        message: &UserMessage,
    ) -> Result<LimiterUpdate, String> {
        // TODO @obelisk: I don't like this unwrap but I don't really know what to do about it
        // I feel like I just have to hope the system never fails to give me the time?
        // Perhaps it's better just to stop this limiter in that event
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

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
