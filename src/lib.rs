#[macro_use]
extern crate log;

pub mod authentication;
pub mod config;
pub mod handler;
pub mod processor;
pub mod presentation;
pub mod ratelimiting;
pub mod ws;

use std::sync::Arc;

pub use presentation::Presentation;

use dashmap::DashMap;
use ratelimiting::RatelimiterResponse;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use warp::{Rejection, filters::ws::Message};

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
    user_message: IncomingMessage,
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
pub enum OutgoingPresenterMessage {
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
pub enum IncomingMessage {
    Emoji(EmojiMessage),
    NewSlide(NewSlideMessage),
}

#[derive(Debug, Serialize)]
pub enum OutgoingUserMessage {
    RatelimiterResponse(RatelimiterResponse),
    NewSlide(SlideSettings),
}

impl OutgoingUserMessage {
    pub fn json(&self) -> String {
        match serde_json::to_string(&self) {
            Ok(text) => text,
            Err(e) => {
                error!("Could not serialize outgoing user message: {e}");
                String::new()
            }
        }
    }
}

impl std::fmt::Display for IncomingMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IncomingMessage::Emoji(emoji) => write!(
                f,
                "{} for {} with size {}",
                emoji.emoji, emoji.slide, emoji.size
            ),
            IncomingMessage::NewSlide(slide) => write!(
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
    pub pid: String, // Presentation ID, should always match the kid in header to be valid
    pub exp: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientJoinPresentationData {
    pub presentation: String,
    pub claims: JwtClaims,
}

