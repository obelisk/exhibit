use dashmap::DashMap;
use serde::Deserialize;

use crate::{IncomingUserMessage, User};

use super::{Limiter, LimiterUpdate};

#[derive(Deserialize, Clone, Debug)]
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
        last_message_time: u64,
        current_time: u64,
        _data_prefix: &str,
        _data: &DashMap<String, u64>,
        user: &User,
        _: &IncomingUserMessage,
    ) -> Result<LimiterUpdate, String> {
        let identity = &user.identity;
        // If they've never sent a message then it's effectively 0

        if last_message_time > current_time {
            error!(
                "{} last sent an emoji in the future. Not allowing this new send",
                identity
            );

            return Err("Try again shortly.".to_string());
        }

        // Check if this message should be blocked
        if (current_time - last_message_time) < self.interval {
            return Err(format!(
                "Try again in {} seconds",
                self.interval - (current_time - last_message_time)
            ));
        }

        // Last message time is stored in global limiter scope so we don't need to return anything
        Ok(LimiterUpdate {
            client_message: format!("Next send allowed: {}", current_time + self.interval),
            limiter_data_update: None,
        })
    }
}
