use serde::{Deserialize, Serialize};

use crate::{SlideSettings, Presenter, User};

pub mod presenter;
pub mod user;

pub use presenter::*;
pub use user::*;

pub trait OutgoingMessage: Clone {}

#[derive(Debug, Deserialize)]
pub enum IncomingMessage {
    Presenter(presenter::IncomingPresenterMessage),
    User(user::IncomingUserMessage),
}

#[derive(Debug)]
pub struct IdentifiedIncomingMessage<T: OutgoingMessage> {
    pub client: T,
    pub message: IncomingMessage,
}

impl std::fmt::Display for IdentifiedIncomingMessage<Presenter> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.client.identity, self.client.guid, self.message
        )
    }
}

impl std::fmt::Display for IdentifiedIncomingMessage<User> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.client.identity, self.client.guid, self.message
        )
    }
}


#[derive(Clone, Debug, Deserialize, Serialize)]
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

