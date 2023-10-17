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
use jsonwebtoken::DecodingKey;
use ratelimiting::Ratelimiter;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{self},
    RwLock,
};
use warp::{ws::Message, Rejection};

// A user can be connected on multiple devices so we have a hashmap
// linking their identity to another hashmap of their connected
// devices
pub type Clients = Arc<DashMap<String, Client>>;
pub type Presenters = Arc<DashMap<String, Client>>;
pub type Presentations = Arc<DashMap<String, Presentation>>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub identity: String,
    pub guid: String,
    pub presentation: String,
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

#[derive(Debug, Serialize)]
pub enum BroadcastMessage {
    Emoji(EmojiMessage),
    NewSlide(SlideSettings),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
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
    pub pid: String, // Presentation ID, should always match kid in header to be valid
    pub exp: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientJoinPresentationData {
    pub presentation: String,
    pub claims: JwtClaims,
}

#[derive(Clone)]
pub struct Presentation {
    id: String,
    presenter_identity: String,
    pub clients: Clients,
    pub presenters: Presenters,
    pub authentication_key: DecodingKey,
    pub ratelimiter: Ratelimiter,
    pub slide_settings: Arc<RwLock<Option<SlideSettings>>>,
    pub encrypted: bool,
}

impl Presentation {
    pub fn new(
        presentation_id: String,
        presenter_identity: String,
        encrypted: bool,
        authentication_key: DecodingKey,
    ) -> Self {
        Self {
            id: presentation_id,
            presenter_identity,
            clients: Arc::new(DashMap::new()),
            presenters: Arc::new(DashMap::new()),
            // TODO @obelisk: I bet this doesn't use secure randomness.
            // Double check
            authentication_key,
            ratelimiter: Ratelimiter::new(),
            slide_settings: Arc::new(None.into()),
            encrypted,
        }
    }
}
