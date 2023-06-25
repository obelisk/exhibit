#[macro_use]
extern crate log;

pub mod handler;
pub mod ws;

use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use tokio::sync::{mpsc, RwLock};
use warp::{ws::Message, Rejection};

// A user can be connected on multiple devices so we have a hashmap
// linking their identity to another hashmap of their connected
// devices
pub type Clients = Arc<RwLock<HashMap<String, HashMap<String, Client>>>>;
pub type Result<T> = std::result::Result<T, Rejection>;

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub emoji_sender: mpsc::UnboundedSender<EmojiMessage>,
}

pub struct EmojiMessage {
    pub identity: String,
    pub slide: u64,
    pub emoji: String,
}

#[derive(Debug, Deserialize)]
pub enum UserMessage {
    Emoji { slide: u64, emoji: String },
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
