use serde::{Serialize, Deserialize};

use crate::{EmojiMessage, NewSlideMessage};


#[derive(Debug, Serialize)]
pub enum OutgoingPresenterMessage {
    Emoji(EmojiMessage),
    //NewSlide(SlideSettings),
}

// pub struct CreatePoleMessage {
//     pub name: String,
//     pub options: Vec<String>,
// }


#[derive(Debug, Deserialize)]
pub enum IncomingPresenterMessage {
    NewSlide(NewSlideMessage),
    //AddRatelimiter
    //RemoveRatelimiter
    //CreatePole(CreatePoleMessage),
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
