use warp::ws::Message;

mod emoji;
mod vote;

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
    info!("Got presenter message: {presenter_message}");
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
            if let Err(existing_poll) = presentation.get_polls().new_poll(poll.clone()) {
                let warn = format!("Presenter tried to create poll that already exists: {:?}", &existing_poll);
                warn!("{warn}");
                presenter.send_ignore_fail(OutgoingPresenterMessage::Error(warn));
                broadcast_to_clients(OutgoingUserMessage::NewPoll(existing_poll), presentation.users).await;
            } else {
                broadcast_to_clients(OutgoingUserMessage::NewPoll(poll), presentation.users).await;
            }
        }
        IncomingPresenterMessage::GetPollTotals(poll) => {
            let results = presentation.get_polls().get_poll_totals(&poll.name);
            if let Some(results) = results {
                presenter.send_ignore_fail(OutgoingPresenterMessage::PollResults(results));
            } else {
                let warn = format!("Presenter requested poll results for a poll that does not exist: {}", poll.name);
                warn!("{warn}");
                presenter.send_ignore_fail(OutgoingPresenterMessage::Error(warn));
            }
        
        },
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
        IncomingUserMessage::Vote(vote) => vote::handle_user_vote(
            &presentation,
            user.clone(),
            vote,
            presentation.presenters.clone(),
        ).await,
    }
}