#[macro_use]
extern crate log;

pub mod authentication;
pub mod config;
pub mod handler;
pub mod processor;
pub mod ratelimiting;
pub mod ws;

use std::sync::Arc;

use dashmap::DashMap;
use ratelimiting::Ratelimiter;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    RwLock,
};
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
    pub guid: String,
}

#[derive(Debug)]
pub struct IdentifiedUserMessage {
    client: Client,
    user_message: UserMessage,
}

impl std::fmt::Display for IdentifiedUserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.client.identity, self.client.guid, self.user_message
        )
    }
}

#[derive(Debug, Clone)]
pub struct Presenter {
    pub sender: mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>,
}

#[derive(Debug, Serialize)]
pub enum BroadcastMessage {
    Emoji(EmojiMessage),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EmojiMessage {
    slide: u64,
    emoji: String,
    size: u8,
}

#[derive(Debug, Deserialize)]
pub struct NewSlideMessage {
    slide: u64,
    slide_settings: SlideSettings,
}

#[derive(Debug, Deserialize)]
pub enum UserMessage {
    Emoji(EmojiMessage),
    NewSlide(NewSlideMessage),
}

impl std::fmt::Display for UserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserMessage::Emoji(emoji) => write!(
                f,
                "{} for {} with size {}",
                emoji.emoji, emoji.slide, emoji.size
            ),
            UserMessage::NewSlide(slide) => write!(
                f,
                "New settings for slide {}: {}",
                slide.slide, slide.slide_settings
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SlideSettings {
    pub message: String,
    pub emojis: Vec<String>,
}

impl std::fmt::Display for SlideSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} - {}", self.emojis, self.message)
    }
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
    pub ratelimiter: Ratelimiter,
    pub slide_settings: Arc<RwLock<Option<SlideSettings>>>,
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
            ratelimiter: Ratelimiter::new(),
            slide_settings: Arc::new(None.into()),
        }
    }
}
