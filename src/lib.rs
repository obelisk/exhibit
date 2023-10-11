#[macro_use]
extern crate log;

pub mod authentication;
pub mod config;
pub mod handler;
pub mod processor;
pub mod ratelimiting;
pub mod ws;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;
use warp::{ws::Message, Rejection};

// A user can be connected on multiple devices so we have a hashmap
// linking their identity to another hashmap of their connected
// devices
pub type Clients = DashMap<String, Client>;
pub type Presenters = DashMap<String, Presenter>;
pub type Presentations = DashMap<String, Presentation>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub identity: String,
}

#[derive(Debug, Clone)]
pub struct Presenter {
    pub sender: mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>,
}

#[derive(Debug, Serialize)]
pub struct EmojiMessage {
    pub identity: String,
    pub slide: u64,
    pub emoji: String,
    pub size: u8,
}

#[derive(Debug)]
pub struct IdentifiedUserMessage {
    identity: String,
    guid_identifier: String,
    clients: Clients,
    user_message: UserMessage,
}

#[derive(Debug, Deserialize)]
pub enum UserMessage {
    Emoji { slide: u64, emoji: String, size: u8 },
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlideSettings {
    pub message: String,
    pub emojis: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub enum ConfigurationMessage {
    NewSlide {
        slide: u64,
        slide_settings: SlideSettings,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // Contains the user's identifying information
    pub exp: usize,
    pub aud: String,
    pub kid: String,
}

#[derive(Clone)]
pub struct Presentation {
    id: String,
    owner: String,
    pub clients: Clients,
    pub presenters: Presenters,
    pub client_authentication_key: String,
}

impl Presentation {
    pub fn new(owner: String) -> Self {
        Self {
            id: Uuid::new_v4().as_simple().to_string(),
            owner,
            clients: DashMap::new(),
            presenters: DashMap::new(),
            // TODO @obelisk: I bet this doesn't use secure randomness.
            // Double check
            client_authentication_key: Uuid::new_v4().as_simple().to_string(),
        }
    }
}
