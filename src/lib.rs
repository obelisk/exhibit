#[macro_use]
extern crate log;

pub mod authentication;
pub mod config;
pub mod handler;
pub mod processor;
pub mod ratelimiting;
pub mod ws;

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use warp::{ws::Message, Rejection};

// A user can be connected on multiple devices so we have a hashmap
// linking their identity to another hashmap of their connected
// devices
pub type Clients = Arc<RwLock<HashMap<String, Client>>>;
pub type Presenters = Arc<RwLock<HashMap<String, Presenter>>>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub emoji_sender: mpsc::UnboundedSender<IdentifiedUserMessage>,
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
}

#[derive(Clone)]
pub struct Presentation {
    pub id: String,
    pub clients: Clients,
    pub presenters: Presenters,
    pub client_authentication_key: zeroize::Zeroizing<String>,
}
