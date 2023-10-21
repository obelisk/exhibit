use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::mpsc::UnboundedReceiver;
use warp::ws::Message;

mod emoji;

use crate::{
    ratelimiting::RatelimiterResponse, OutgoingPresenterMessage, Clients, IdentifiedUserMessage,
    Presentation, Presentations, Presenters, IncomingMessage, OutgoingUserMessage,
};

#[derive(Serialize)]
struct ClientRateLimitResponse {
    ratelimit_status: HashMap<String, String>,
}

pub async fn broadcast_to_presenters(message: OutgoingPresenterMessage, presenters: Presenters) {
    let event = serde_json::to_string(&message).unwrap();
    presenters.iter().for_each(|item| {
        let connected_presenter = item.value();
        if let Some(ref connected_presenter) = connected_presenter.sender {
            let _ = connected_presenter.send(Ok(Message::text(&event)));
        }
    });
}

pub async fn broadcast_to_clients(message: OutgoingPresenterMessage, clients: Clients) {
    let event = serde_json::to_string(&message).unwrap();
    clients.iter().for_each(|item| {
        let connected_client = item.value();
        if let Some(ref connected_client) = connected_client.sender {
            let _ = connected_client.send(Ok(Message::text(&event)));
        }
    });
}

/// Handle a specific incoming user message. Each message is handled in it's own
/// tokio task so we don't need to start any new ones to prevent blocking, only
/// to achieve concurrency
async fn handle_user_message(user_message: IdentifiedUserMessage, mut presentation: Presentation) {

    // Check ratelimit for all users except the configured presenter
    if user_message.client.identity != presentation.presenter_identity {
        // Run the ratelimiter check
        let ratelimiter_response = presentation.ratelimiter.check_allowed(&user_message);

        // If the connection is still open (should be almost always), send the response
        if let Some(ref sender) = user_message.client.sender {
            let response = OutgoingUserMessage::RatelimiterResponse(ratelimiter_response.clone()).json();
            let _ = sender.send(Ok(Message::text(response)));
        } else {
            error!("{} sent a message from a guid that has no open connection. Dropping message: {user_message}", user_message.client.identity);
            return;
        }

        // If something in the system blocked them, log it and stop
        if let RatelimiterResponse::Blocked(name) = ratelimiter_response {
            warn!(
                "{} sent a message but was blocked by the ratelimiter: {name}",
                user_message.client.identity
            );
            return;
        }
    }

    // Now that we've dealt with the ratelimiting, we can handle the message
    match user_message.user_message {
        IncomingMessage::Emoji(msg) => {
            emoji::handle_user_emoji(
                &presentation,
                user_message.client.clone(),
                msg,
                presentation.presenters.clone(),
            )
            .await
        }
        IncomingMessage::NewSlide(msg) => {
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
            *slide_settings = Some(msg.slide_settings.clone());

            broadcast_to_clients(
                OutgoingPresenterMessage::NewSlide(msg.slide_settings),
                presentation.clients,
            )
            .await;
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
