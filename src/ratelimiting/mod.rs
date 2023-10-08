use std::collections::HashMap;
use std::sync::Arc;

use concurrent_map::ConcurrentMap;

use crate::IdentifiedUserMessage;

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
        current_time: u64,
        data_prefix: &str,
        data: &ConcurrentMap<String, u64>,
        message: &IdentifiedUserMessage,
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

    pub fn check_allowed(&mut self, message: &IdentifiedUserMessage) -> RatelimiterResponse {
        // TODO @obelisk: I don't like this unwrap but I don't really know what to do about it
        // I feel like I just have to hope the system never fails to give me the time?
        // Perhaps it's better just to stop this limiter in that event
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut updates: HashMap<String, LimiterUpdate> = HashMap::new();
        for (name, limiter) in self.limiters.iter() {
            let update = limiter.check_allowed(current_time, &name, &self.data, message);
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
        for (name, update) in updates {
            self.data
                .insert(format!("{name}-{}", update.data), update.value);
        }

        return RatelimiterResponse {
            blocked: false,
            blocker: String::new(),
        };
    }
}
