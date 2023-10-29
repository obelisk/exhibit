use warp::ws::Message;

mod emoji;

use crate::{
    ratelimiting::RatelimiterResponse, OutgoingPresenterMessage, Users,
    Presentation, Presenters, OutgoingUserMessage, IncomingPresenterMessage, IncomingUserMessage, Presenter, User,
};

pub async fn broadcast_to_presenters(message: OutgoingPresenterMessage, presenters: Presenters) {
    let event = serde_json::to_string(&message).unwrap();
    presenters.iter().for_each(|item| {
        let connected_presenter = item.value();
        if let Some(ref connected_presenter) = connected_presenter.sender {
            let _ = connected_presenter.send(Ok(Message::text(&event)));
        }
    });
}

pub async fn broadcast_to_clients(message: OutgoingUserMessage, users: Users) {
    let event = serde_json::to_string(&message).unwrap();
    users.iter().for_each(|item| {
        let connected_client = item.value();
        if let Some(ref connected_client) = connected_client.sender {
            let _ = connected_client.send(Ok(Message::text(&event)));
        }
    });
}

pub async fn handle_presenter_message_types(presenter_message: IncomingPresenterMessage, presenter: Presenter, presentation: Presentation) {
    match presenter_message {
        IncomingPresenterMessage::NewSlide(msg) => {
            let mut slide_settings = presentation.slide_settings.write().await;
            *slide_settings = Some(msg.slide_settings.clone());

            broadcast_to_clients(
                OutgoingUserMessage::NewSlide(msg.slide_settings),
                presentation.users,
            )
            .await;
        }
        IncomingPresenterMessage::NewPoll(poll) => {
            if let Err(msg) = presentation.new_poll(poll.name) {
                warn!("{msg}");
                presenter.send_ignore_fail(OutgoingPresenterMessage::Error(msg));
            }
        }
        IncomingPresenterMessage::GetPollResults(_) => todo!(),
    }
}

pub async fn handle_user_message_types(user_message: IncomingUserMessage, user: User, presentation: Presentation) {
    // Run the ratelimiter check
    let ratelimiter_response = presentation.ratelimiter.check_allowed(user.clone(), &user_message);

    // If the connection is still open (should be almost always), send the response
    if let Some(ref sender) = user.sender {
        let response = OutgoingUserMessage::RatelimiterResponse(ratelimiter_response.clone()).json();
        let _ = sender.send(Ok(Message::text(response)));
    } else {
        error!("{} sent a message from a guid that has no open connection. Dropping message: {user_message}", user.identity);
        return;
    }

    // If something in the system blocked them, log it and stop
    if let RatelimiterResponse::Blocked(name) = ratelimiter_response {
        warn!(
            "{} sent a message but was blocked by the ratelimiter: {name}",
            user.identity
        );
        return;
    }

    match user_message {
        IncomingUserMessage::Emoji(msg) => 
            emoji::handle_user_emoji(
                &presentation,
                user.clone(),
                msg,
                presentation.presenters.clone(),
            )
            .await,
    }
}