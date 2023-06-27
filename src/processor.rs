use std::collections::HashMap;

use tokio::sync::mpsc::UnboundedReceiver;
use warp::ws::Message;

use crate::{ConfigurationMessage, EmojiMessage, Presenters, SlideSettings};

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

pub async fn handle_sent_emojis(
    mut emoji_receiver: UnboundedReceiver<EmojiMessage>,
    mut configuration_receiver: UnboundedReceiver<ConfigurationMessage>,
    presenters: Presenters,
) {
    // Stores the settings for the slides we've seen so far
    // to allow people to send emojis for previous slides
    let mut all_slide_settings: HashMap<u64, SlideSettings> = HashMap::new();

    // Keep track of the last time a user sent an emoji to rate limit them
    let mut rate_limiter: HashMap<String, u64> = HashMap::new();
    loop {
        tokio::select! {
            msg = emoji_receiver.recv() => {
                let msg = match msg {
                    Some(m) => m,
                    None => break,
                };

                // Check that this slide exists
                let settings = match all_slide_settings.get(&msg.slide) {
                    Some(s) => s,
                    None => {
                        error!("{} sent {} for unknown slide {}", msg.identity, msg.emoji, msg.slide);
                        continue;
                    },
                };

                // Check that they are sending a valid emoji for the current slide
                if !settings.emojis.contains(&msg.emoji) {
                    error!("{} sent invalid {} for slide {}", msg.identity, msg.emoji, msg.slide);
                    continue;
                }

                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();


                // This has the effect of constantly blocking spammers
                // because even though it's not sent, it still resets the timer
                // Not sure if this is good or bad
                let allowed_to_send = if let Some(previous_send) = rate_limiter.get_mut(&msg.identity) {
                    // It has been more than 10 seconds since this user last sent an emoji
                    if time - *previous_send > 10 {
                        *previous_send = time;
                        true
                    } else {
                        // They have not waited 10 seconds
                        false
                    }
                } else {
                    // This is the first emoji sent by this user
                    rate_limiter.insert(msg.identity.clone(), time);
                    true
                };

                if allowed_to_send {
                    info!("{} sent {}", msg.identity, msg.emoji);
                    let presenters = presenters.clone();
                    tokio::task::spawn(async move {
                        broadcast_to_presenters(msg, presenters).await;
                    });

                } else {
                    error!("{} tried to send {} too soon", msg.identity, msg.emoji);
                }

            }
            config = configuration_receiver.recv() => {
                match config {
                    Some(ConfigurationMessage::NewSlide { slide, slide_settings }) => {
                        info!("New slide: {slide}, Message: {}, Emojis: {}", slide_settings.message, slide_settings.emojis.join(","));
                        all_slide_settings.insert(slide, slide_settings);
                    },
                    None => break,
                }
            }
        };
    }
}
