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
    // What are the settings for the current slide
    let mut settings: Option<SlideSettings> = None;

    // Store the number of current slide
    let mut current_slide_number = None;

    // Keep track of the last time a user sent an emoji to rate limit them
    let mut rate_limiter: HashMap<String, u64> = HashMap::new();

    loop {
        tokio::select! {
            msg = emoji_receiver.recv() => {
                let msg = match msg {
                    Some(m) => m,
                    None => break,
                };

                // Check if the presentation has started
                let slide_settings = if let Some(ref s) = settings {
                    s
                } else {
                    error!("{} sent {} for slide {} but presentation has not started", msg.identity, msg.emoji, msg.slide);
                    continue;
                };

                // Check that they are sending a valid emoji for the current slide
                if !slide_settings.emojis.contains(&msg.emoji) {
                    error!("{} sent invalid {} for slide {}", msg.identity, msg.emoji, msg.slide);
                    continue;
                }

                // TODO @obelisk: I don't like this unwrap but I don't really know what to do about it
                // I feel like I just have to hope the system never fails to give me the time?
                // Perhaps it's better just to stop ratelimiting in the unlikely event we stop getting the time
                let time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Implement ratelimiting
                let allowed_to_send = if let Some(previous_send) = rate_limiter.get_mut(&msg.identity) {
                    if *previous_send > time {
                        error!("{} last sent an emoji in the future. Not allowing this new send", msg.identity);
                        continue;
                    }
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
                    warn!("{} tried to send {} too soon", msg.identity, msg.emoji);
                }

            }
            config = configuration_receiver.recv() => {
                match config {
                    Some(ConfigurationMessage::NewSlide { slide, slide_settings }) => {
                        info!("New current slide set: {slide}, Message: {}, Emojis: {}", slide_settings.message, slide_settings.emojis.join(","));
                        current_slide_number = Some(slide);
                        settings = Some(slide_settings);
                    },
                    None => break,
                }
            }
        };
    }
}
