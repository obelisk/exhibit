use serde::{Deserialize, Serialize};

use crate::{ratelimiting::RatelimiterResponse, SlideSettings, Client};


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


#[derive(Debug, Serialize)]
pub enum OutgoingPresenterMessage {
    Emoji(EmojiMessage),
    NewSlide(SlideSettings),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EmojiMessage {
    pub slide: u64,
    pub emoji: String,
    pub size: u8,
}

#[derive(Debug, Deserialize)]
pub struct NewSlideMessage {
    pub slide: u64,
    pub slide_settings: SlideSettings,
}

#[derive(Debug, Deserialize)]
pub enum IncomingMessage {
    Presenter(IncomingPresenterMessage),
    User(IncomingUserMessage),
}

impl std::fmt::Display for IncomingMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}


#[derive(Debug, Deserialize)]
pub enum IncomingPresenterMessage {
    NewSlide(NewSlideMessage),
}

impl std::fmt::Display for IncomingPresenterMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewSlide(slide) => write!(
                f,
                "New settings for slide {}: {}",
                slide.slide, slide.slide_settings
            ),
        }
    }
}


#[derive(Debug, Deserialize)]
pub enum IncomingUserMessage {
    Emoji(EmojiMessage),
}

impl std::fmt::Display for IncomingUserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emoji(emoji) => write!(
                f,
                "{} for {} with size {}",
                emoji.emoji, emoji.slide, emoji.size
            )
        }
    }
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