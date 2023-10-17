use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;
use warp::ws::Message;

mod emoji;

use crate::{
    ratelimiting::{time::TimeLimiter, value::ValueLimiter, Ratelimiter, RatelimiterResponse},
    BroadcastMessage, EmojiMessage, IdentifiedUserMessage, Presentation, Presentations, Presenters,
    SlideSettings, UserMessage,
};

#[derive(Serialize)]
struct ClientRateLimitResponse {
    ratelimit_status: HashMap<String, String>,
}

pub async fn broadcast_to_presenters(message: BroadcastMessage, presenters: Presenters) {
    let event = serde_json::to_string(&message).unwrap();
    presenters.iter().for_each(|item| {
        let connected_presenter = item.value();
        if let Some(ref connected_presenter) = connected_presenter.sender {
            let _ = connected_presenter.send(Ok(Message::text(&event)));
        }
    });
}

async fn handle_user_message(user_message: IdentifiedUserMessage, mut presentation: Presentation) {
    let ratelimit_responses = match presentation.ratelimiter.check_allowed(&user_message) {
        RatelimiterResponse::Allowed(responses) => responses,
        RatelimiterResponse::Blocked(blocker) => {
            warn!("{user_message} was blocked by {blocker}");
            return;
        }
    };

    match user_message.user_message {
        UserMessage::Emoji(msg) => {
            emoji::handle_user_emoji(
                &presentation,
                ratelimit_responses,
                user_message.client.clone(),
                msg,
                presentation.presenters.clone(),
            )
            .await
        }
        UserMessage::NewSlide(msg) => {
            // Check if the client sending this message has the identity of the
            // set presenter.
            if user_message.client.identity != presentation.presenter_identity {
                warn!(
                    "{} attempted to change slide data but only {} is allowed to do that",
                    user_message.client.identity, presentation.presenter_identity
                );
                return;
            }

            // The message is from the presenter identity
            let mut slide_settings = presentation.slide_settings.write().await;
            *slide_settings = Some(msg.slide_settings);
        }
    };
}

pub async fn handle_sent_messages(
    mut user_message_receiver: UnboundedReceiver<IdentifiedUserMessage>,
    presentations: Presentations,
) {
    loop {
        tokio::select! {
            msg = user_message_receiver.recv() => {
                let msg = match msg {
                    Some(m) => m,
                    None => break,
                };

                if let Some(presentation) = presentations.get(&msg.client.presentation) {
                    tokio::spawn({
                        handle_user_message(msg, presentation.value().clone())
                    });
                } else {
                    warn!("{} send a message for presentation {} which doesn't exist: {msg}", msg.client.identity, msg.client.presentation);
                }
            }
        };
    }
}
