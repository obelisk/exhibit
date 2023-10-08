use std::sync::Arc;

use tokio::sync::mpsc::UnboundedReceiver;
use warp::ws::Message;

use crate::{
    ratelimiting::{time::TimeLimiter, value::ValueLimiter, Ratelimiter},
    ConfigurationMessage, EmojiMessage, IdentifiedUserMessage, Presenters, SlideSettings,
    UserMessage,
};

pub async fn broadcast_to_presenters(message: EmojiMessage, presenters: Presenters) {
    let event = serde_json::to_string(&message).unwrap();
    presenters
        .read()
        .await
        .iter()
        .for_each(|(_, connected_presenter)| {
            let _ = connected_presenter.sender.send(Ok(Message::text(&event)));
        });
}

async fn handle_user_message(
    mut rate_limiter: Ratelimiter,
    presenters: Presenters,
    slide_settings: Option<SlideSettings>,
    user_message: IdentifiedUserMessage,
) {
    let identity = user_message.identity.clone();
    // Check if the presentation has started
    let slide_settings = if let Some(ref s) = slide_settings {
        s
    } else {
        error!("{identity} sent a message but the presentation has not started");
        return;
    };

    let allowed_to_send = rate_limiter.check_allowed(&user_message);

    match user_message.user_message {
        UserMessage::Emoji { slide, emoji } => {
            // Check that they are sending a valid emoji for the current slide
            if !slide_settings.emojis.contains(&emoji) {
                error!("{identity} sent invalid {emoji} for slide {slide}");
                return;
            }

            if !allowed_to_send.blocked {
                info!("{identity} sent {emoji}");
                tokio::task::spawn(async move {
                    broadcast_to_presenters(
                        EmojiMessage {
                            identity,
                            slide,
                            emoji,
                        },
                        presenters,
                    )
                    .await;
                });
            } else {
                warn!(
                    "Ratelimiter {} blocked from {identity} sending {emoji}",
                    allowed_to_send.blocker
                );
            }
        }
    };
}

pub async fn handle_sent_messages(
    mut user_message_receiver: UnboundedReceiver<IdentifiedUserMessage>,
    mut configuration_receiver: UnboundedReceiver<ConfigurationMessage>,
    presenters: Presenters,
) {
    // What are the settings for the current slide
    let mut settings: Option<SlideSettings> = None;

    // Keep track of the last time a user sent an emoji to rate limit them
    let mut rate_limiter = Ratelimiter::new();
    rate_limiter.add_ratelimit("20s-limit".to_string(), Arc::new(TimeLimiter::new(20)));
    rate_limiter.add_ratelimit(
        "only-send-3".to_string(),
        Arc::new(ValueLimiter::new(1, 1, 1, 3)),
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
            config = configuration_receiver.recv() => {
                match config {
                    Some(ConfigurationMessage::NewSlide { slide_settings, .. }) => {
                        info!("New slide set, Message: {}, Emojis: {}", slide_settings.message, slide_settings.emojis.join(","));
                        settings = Some(slide_settings);
                    },
                    None => break,
                }
            }
        };
    }
}
