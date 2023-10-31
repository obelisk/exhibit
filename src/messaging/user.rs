use serde::{Serialize, Deserialize};

use crate::{ratelimiting::RatelimiterResponse, SlideSettings, EmojiMessage, OutgoingMessage, Vote, NewPollMessage};


#[derive(Debug, Deserialize)]
pub enum IncomingUserMessage {
    Emoji(EmojiMessage),
    Vote(Vote),
}

impl std::fmt::Display for IncomingUserMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emoji(emoji) => write!(
                f,
                "{} with size {}",
                emoji.emoji, emoji.size
            ),
            Self::Vote(vote) => write!(
                f,
                "Vote for {:?}",
                vote
            ),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub enum OutgoingUserMessage {
    InitialPresentationData {title: String, settings: Option<SlideSettings>},
    RatelimiterResponse(RatelimiterResponse),
    NewSlide(SlideSettings),
    NewPoll(NewPollMessage),
    Success(String),
    Error(String),
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