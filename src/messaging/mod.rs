use serde::{Deserialize, Serialize};

use crate::{SlideSettings, Client};

pub mod presenter;
pub mod user;

pub use presenter::*;
pub use user::*;


#[derive(Debug, Deserialize)]
pub enum IncomingMessage {
    Presenter(presenter::IncomingPresenterMessage),
    User(user::IncomingUserMessage),
}

#[derive(Debug)]
pub struct IdentifiedIncomingMessage {
    pub client: Client,
    pub message: IncomingMessage,
}

impl std::fmt::Display for IdentifiedIncomingMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.client.identity, self.client.guid, self.message
        )
    }
}


#[derive(Debug, Deserialize, Serialize)]
pub struct EmojiMessage {
    pub emoji: String,
    pub size: u8,
}

#[derive(Debug, Deserialize)]
pub struct NewSlideMessage {
    pub slide: u64,
    pub slide_settings: SlideSettings,
}

impl std::fmt::Display for IncomingMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IncomingMessage::Presenter(x) => write!(f, "{x}"),
            IncomingMessage::User(x) => write!(f, "{x}"),
        }
    }
}

