use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use serde::Serialize;

use crate::{Client, IncomingUserMessage, OutgoingUserMessage, User};

pub mod time;
pub mod value;

#[derive(Clone, Debug, Serialize)]
pub enum RatelimiterResponse {
    Allowed(HashMap<String, String>),
    Blocked(String),
}

pub struct LimiterDataUpdate {
    pub data: String,
    pub value: u64,
}

pub struct LimiterUpdate {
    pub client_message: String,
    pub limiter_data_update: Option<LimiterDataUpdate>,
}

/// A limiter is a system of ratelimiting messages
pub trait Limiter: Send + Sync {
    // Checks if the limiter is going to block the action
    fn check_allowed(
        &self,
        last_message_time: u64,
        current_time: u64,
        data_prefix: &str,
        data: &DashMap<String, u64>,
        user: &User,
        message: &IncomingUserMessage,
    ) -> Result<LimiterUpdate, String>;
}

#[derive(Clone)]
pub struct Ratelimiter {
    limiters: DashMap<String, Arc<dyn Limiter>>,
    limiter_data: DashMap<String, u64>,
    global_data: DashMap<String, u64>,
}

impl Default for Ratelimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Ratelimiter {
    pub fn new() -> Self {
        Self {
            // Contains all the configured limiters
            limiters: DashMap::default(),

            // Contains the data for all the configured limiters. Limiters
            // are never given write access to this data and updates must be
            // done by the Ratelimiter
            limiter_data: DashMap::default(),

            // Separated data storage for the ratelimiter itself to store
            // data that is useful to many limiters such as last time a message
            // was successfully sent
            global_data: DashMap::default(),
        }
    }

    /// Adds a ratelimit to the ratelimiter. If a ratelimit with that name
    /// is already present it replaces it.
    pub fn add_ratelimit(&self, name: String, limit: Arc<dyn Limiter>) {
        self.limiters.insert(name, limit);
    }

    /// Remove a limiter from the ratelimiter system
    pub fn remove_ratelimit(&self, name: &str) {
        self.limiters.remove(name);
    }

    pub fn check_allowed(&self, client: Client<OutgoingUserMessage>, message: &IncomingUserMessage) -> RatelimiterResponse {
        // TODO @obelisk: I don't like this unwrap but I don't really know what to do about it
        // I feel like I just have to hope the system never fails to give me the time?
        // Perhaps it's better just to stop this limiter in that event
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        trace!("Size of global data: {}", self.global_data.len());

        let last_message_time = self
            .global_data
            .get(&format!("lmt-{}", client.identity))
            .map(|x| x.to_owned())
            .unwrap_or(0);

        let mut updates: HashMap<String, LimiterUpdate> = HashMap::new();
        for item in self.limiters.iter() {
            let update = item.value().check_allowed(
                last_message_time,
                current_time,
                item.key(),
                &self.limiter_data,
                &client,
                message,
            );
            match update {
                Ok(update) => updates.insert(item.key().to_string(), update),
                Err(e) => return RatelimiterResponse::Blocked(e),
            };
        }

        // Update all the limiters now that none of them are blocking
        for (name, update) in &updates {
            if let Some(ref update) = update.limiter_data_update {
                self.limiter_data
                    .insert(format!("{name}-{}", update.data), update.value);
            }
        }

        // Update global data as well
        self.global_data
            .insert(format!("lmt-{}", client.identity), current_time);

        RatelimiterResponse::Allowed(
            updates
                .into_iter()
                .map(|(name, update)| (name, update.client_message))
                .collect(),
        )
    }
}
