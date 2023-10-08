use std::collections::HashMap;
use std::sync::Arc;

use concurrent_map::ConcurrentMap;

use crate::UserMessage;

pub mod time;
pub mod value;

pub struct RatelimiterResponse {
    pub blocked: bool,
    pub blocker: String,
}

pub struct LimiterUpdate {
    pub data: String,
    pub value: u64,
}

/// A limiter is a system of ratelimiting messages
pub trait Limiter: Send + Sync {
    // Checks if the limiter is going to block the action
    fn check_allowed(
        &self,
        data_prefix: &str,
        data: &ConcurrentMap<String, u64>,
        identity: &str,
        message: &UserMessage,
    ) -> Result<LimiterUpdate, String>;
}

#[derive(Clone)]
pub struct Ratelimiter {
    limiters: ConcurrentMap<String, Arc<dyn Limiter>>,
    data: ConcurrentMap<String, u64>,
}

impl Ratelimiter {
    pub fn new() -> Self {
        return Self {
            limiters: ConcurrentMap::default(),
            data: ConcurrentMap::default(),
        };
    }

    /// Adds a ratelimit to the ratelimiter. If a ratelimit with that name
    /// is already present it replaces it.
    pub fn add_ratelimit(&mut self, name: String, limit: Arc<dyn Limiter>) {
        self.limiters.insert(name, limit);
    }

    pub fn check_allowed(&mut self, identity: &str, message: &UserMessage) -> RatelimiterResponse {
        let mut updates: HashMap<String, LimiterUpdate> = HashMap::new();
        for (name, limiter) in self.limiters.iter() {
            let update = limiter.check_allowed(&name, &self.data, identity, message);
            match update {
                Ok(update) => {
                    updates.insert(name.to_string(), update);
                }
                Err(_) => {
                    return RatelimiterResponse {
                        blocked: true,
                        blocker: name.to_owned(),
                    }
                }
            };
        }

        // Update all the limiters now that none of them are blocking

        return RatelimiterResponse {
            blocked: false,
            blocker: String::new(),
        };
    }
}
