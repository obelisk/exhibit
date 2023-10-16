use std::collections::HashMap;

use warp::filters::ws::Message;

use crate::{
    processor::ClientRateLimitResponse, BroadcastMessage, Client, EmojiMessage, Presenters,
    SlideSettings,
};

pub fn handle_user_emoji(
    slide_settings: &SlideSettings,
    ratelimit_responses: HashMap<String, String>,
    client: Client,
    emoji_message: EmojiMessage,
    presenters: Presenters,
) {
    let emoji = &emoji_message.emoji;
    let identity = &client.identity;
    // Check that they are sending a valid emoji for the current slide
    if !slide_settings.emojis.contains(&emoji) {
        error!("{identity} sent invalid {emoji} for current slide");
        return;
    }

    // Update the client on ratelimits
    let response = match serde_json::to_string(&ClientRateLimitResponse {
        ratelimit_status: ratelimit_responses,
    }) {
        Ok(text) => text,
        Err(e) => {
            error!("Could not serialize ratelimit response for {identity}: {e}");
            return;
        }
    };

    if let Some(ref sender) = client.sender {
        let _ = sender.send(Ok(Message::text(response)));
    } else {
        error!("{identity} sent a message from a guid that has no open connection. Dropping Emoji: {emoji}");
        return;
    }

    // Send the emojis to the presenters
    info!(
        "{identity} sent {emoji} to presentation {}",
        client.presentation
    );
    tokio::task::spawn(async move {
        super::broadcast_to_presenters(BroadcastMessage::Emoji(emoji_message), presenters).await;
    });
}
