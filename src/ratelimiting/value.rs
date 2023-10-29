use dashmap::DashMap;

use crate::{ratelimiting::LimiterDataUpdate, EmojiMessage, IncomingUserMessage, Client, User};

use super::{Limiter, LimiterUpdate};

#[derive(Clone)]
pub struct ValueLimiter {
    small_cost: u64,
    large_cost: u64,
    huge_cost: u64,
    points_per_10: u64,
    max_points: u64,
}

impl ValueLimiter {
    pub fn new(
        small_cost: u64,
        large_cost: u64,
        huge_cost: u64,
        points_per_10: u64,
        max_points: u64,
    ) -> Self {
        Self {
            small_cost,
            large_cost,
            huge_cost,
            points_per_10,
            max_points,
        }
    }
}

impl Limiter for ValueLimiter {
    fn check_allowed(
        &self,
        last_message_time: u64,
        current_time: u64,
        data_prefix: &str,
        data: &DashMap<String, u64>,
        user: &User,
        message: &IncomingUserMessage,
    ) -> Result<LimiterUpdate, String> {
        let identity = &user.identity;
        let message_cost = match &message {
            IncomingUserMessage::Emoji(EmojiMessage { size: 0, .. }) => self.small_cost, // Normal
            IncomingUserMessage::Emoji(EmojiMessage { size: 1, .. }) => self.large_cost, // Large
            IncomingUserMessage::Emoji(EmojiMessage { size: 2, .. }) => self.huge_cost,  // Huge
            IncomingUserMessage::Emoji(EmojiMessage { size, .. }) => {
                return Err(format!("{identity} sent emoji with invalid size: {size}"))
            }
        };
        // If they've never sent a message then it's whatever their starting balance is
        let existing_balance = data
            .get(&format!("{data_prefix}-{identity}"))
            .map(|x| x.to_owned())
            .unwrap_or(self.max_points);

        // LMT is guaranteed to be smaller by RateLimiter
        let new_balance = std::cmp::min(
            self.max_points,
            existing_balance + (((current_time - last_message_time) / 10) * self.points_per_10),
        );

        if message_cost > new_balance {
            return Err("Emoji too expensive".to_string());
        }
        debug!(
            "{identity} has new reaction balance of {}",
            new_balance - message_cost
        );
        Ok(LimiterUpdate {
            client_message: format!("You have {} remaining points", new_balance - message_cost),
            limiter_data_update: Some(LimiterDataUpdate {
                data: identity.to_string(),
                value: new_balance - message_cost,
            }),
        })
    }
}
