use serde::{Serialize, Deserialize};

use crate::{ratelimiting::RatelimiterResponse, SlideSettings, EmojiMessage, OutgoingMessage};


#[derive(Debug, Deserialize)]
pub enum IncomingUserMessage {
    Emoji(EmojiMessage),
}

impl std::fmt::Display for IncomingUserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emoji(emoji) => write!(
                f,
                "{} with size {}",
                emoji.emoji, emoji.size
            )
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum OutgoingUserMessage {
    InitialPresentationData {title: String, settings: Option<SlideSettings>},
    RatelimiterResponse(RatelimiterResponse),
    NewSlide(SlideSettings),
    Disconnected(String),
}

impl OutgoingMessage for OutgoingUserMessage {}

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