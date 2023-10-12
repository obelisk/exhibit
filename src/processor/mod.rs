use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;
use warp::ws::Message;

mod emoji;

use crate::{
    ratelimiting::{time::TimeLimiter, value::ValueLimiter, Ratelimiter, RatelimiterResponse},
    BroadcastMessage, EmojiMessage, IdentifiedUserMessage, Presenters, SlideSettings, UserMessage,
};

#[derive(Serialize)]
struct ClientRateLimitResponse {
    ratelimit_status: HashMap<String, String>,
}

pub async fn broadcast_to_presenters(message: BroadcastMessage, presenters: Presenters) {
    let event = serde_json::to_string(&message).unwrap();
    presenters.iter().for_each(|item| {
        let connected_presenter = item.value();
        let _ = connected_presenter.sender.send(Ok(Message::text(&event)));
    });
}

async fn handle_user_message(
    mut rate_limiter: Ratelimiter,
    presenters: Presenters,
    slide_settings: Option<SlideSettings>,
    user_message: IdentifiedUserMessage,
) {
    let identity = user_message.client.identity.clone();
    // Check if the presentation has started
    let slide_settings = if let Some(ref s) = slide_settings {
        s
    } else {
        error!("{identity} sent a message but the presentation has not started");
        return;
    };

    let ratelimit_responses = match rate_limiter.check_allowed(&user_message) {
        RatelimiterResponse::Allowed(responses) => responses,
        RatelimiterResponse::Blocked(blocker) => {
            warn!("{user_message} was blocked by {blocker}");
            return;
        }
    };

    match user_message.user_message {
        UserMessage::Emoji(msg) => emoji::handle_user_emoji(
            &slide_settings,
            ratelimit_responses,
            user_message.client.clone(),
            msg,
            presenters.clone(),
        ),
        UserMessage::NewSlide(_msg) => todo!(),
    };
}

pub async fn handle_sent_messages(
    mut user_message_receiver: UnboundedReceiver<IdentifiedUserMessage>,
    presenters: Presenters,
) {
    // What are the settings for the current slide
    let mut settings: Option<SlideSettings> = None;

    // Keep track of the last time a user sent an emoji to rate limit them
    let mut rate_limiter = Ratelimiter::new();
    rate_limiter.add_ratelimit("10s-limit".to_string(), Arc::new(TimeLimiter::new(10)));
    rate_limiter.add_ratelimit(
        "normal-big-huge".to_string(),
        Arc::new(ValueLimiter::new(0, 5, 10, 1, 25)),
    );

    loop {
        tokio::select! {
            msg = user_message_receiver.recv() => {
                let msg = match msg {
                    Some(m) => m,
                    None => break,
                };

                let rate_limiter = rate_limiter.clone();
                let settings = settings.clone();
                tokio::spawn({
                    handle_user_message(rate_limiter, presenters.clone(), settings, msg)
                });
            }
            // config = configuration_receiver.recv() => {
            //     match config {
            //         Some(ConfigurationMessage::NewSlide { slide_settings, .. }) => {
            //             info!("New slide set, Message: {}, Emojis: {}", slide_settings.message, slide_settings.emojis.join(","));
            //             settings = Some(slide_settings);
            //         },
            //         None => break,
            //     }
            // }
        };
    }
}
