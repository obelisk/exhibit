use serde::{Serialize, Deserialize};
use warp::filters::ws::Message;

use crate::{EmojiMessage, NewSlideMessage};


#[derive(Debug, Serialize)]
pub enum OutgoingPresenterMessage {
    Emoji(EmojiMessage),
    Error(String),
    //NewSlide(SlideSettings),
}

impl OutgoingPresenterMessage {
    pub fn json(&self) -> String {
        match serde_json::to_string(&self) {
            Ok(text) => text,
            Err(e) => {
                error!("Could not serialize outgoing user message: {e}");
                String::new()
            }
        }
    }

    pub fn to_sendable_message(&self) -> Message {
        Message::text(self.json())
    }
}

#[derive(Debug, Deserialize)]
pub struct NewPollMessage {
    pub name: String,
    pub options: Vec<String>,
}


#[derive(Debug, Deserialize)]
pub enum IncomingPresenterMessage {
    NewSlide(NewSlideMessage),
    NewPoll(NewPollMessage),
    //AddRatelimiter
    //RemoveRatelimiter
}

impl std::fmt::Display for IncomingPresenterMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewSlide(slide) => write!(
                f,
                "New settings for slide {}: {}",
                slide.slide, slide.slide_settings
            ),
            Self::NewPoll(poll) => write!(
                f,
                "New poll: {} with options {:?}",
                poll.name, poll.options
            )
        }
    }
}
