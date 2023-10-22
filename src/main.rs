use dashmap::DashMap;
use exhibit::authentication::join_presentation;
use exhibit::{
    authentication::new_presentation, config, handler, processor, IdentifiedIncomingMessage,
    Presentations,
};
use tokio::sync::mpsc::unbounded_channel;

use std::net::SocketAddr;
use std::str::FromStr;
use std::{convert::Infallible, sync::Arc};
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();
    let configuration = config::load_configuration();

    // Probably the most important data structure in the whole application.
    // Stores all the presenters and clients for all active presentations
    let presentations: Presentations = Arc::new(DashMap::new());

    let (user_message_sender, user_message_receiver) = unbounded_channel::<IdentifiedIncomingMessage>();

    // APIs
    let health_route = warp::path!("health").and_then(handler::health_handler);
    let presentation_capture = presentations.clone();
    let client_ws_route = warp::path!("ws" / String / String)
        .and(warp::ws())
        .and(with(presentation_capture.clone()))
        .and(with(user_message_sender.clone()))
        .and_then(handler::ws_handler);

    let presentation_capture = presentations.clone();
    let new_presentation = warp::path!("new")
        .and(warp::post())
        // Set maximum request size
        .and(warp::body::content_length_limit(1024 * 4))
        .and(warp::body::form().and_then(move |provided_token| {
            new_presentation(
                configuration.new_presentation_signing_key.clone(),
                provided_token,
                presentation_capture.clone(),
            )
        }))
        .and(with(presentations.clone()))
        .and_then(handler::new_presentation_hander);

    let presentation_capture = presentations.clone();
    let join_route = warp::path!("join")
        .and(warp::post())
        // Set maximum request size
        .and(warp::body::content_length_limit(1024 * 2))
        .and(warp::body::bytes().and_then(move |provided_token| {
            join_presentation(provided_token, presentation_capture.clone())
        }))
        .and(with(presentations.clone()))
        .and_then(handler::join_jwt_handler);

    // SPAs
    let join_spa = warp::path::end().and(warp::fs::file("web/join.html"));
    let presenter_spa = warp::path("present").and(warp::fs::file("web/present.html"));
    let new_spa = warp::path("new").and(warp::fs::file("web/new.html"));

    let all_routes = health_route
        .or(new_presentation)
        .or(join_route)
        .or(client_ws_route)
        .or(join_spa)
        .or(presenter_spa)
        .or(new_spa);

    let presentations_clone = presentations.clone();
    tokio::task::spawn(async move {
        processor::handle_sent_messages(user_message_receiver, presentations_clone).await;

        panic!("User message receiver was dropped?");
    });

    let service_address = SocketAddr::from_str(&format!("{}:{}",configuration.service_address, configuration.service_port)).unwrap();

    warp::serve(all_routes).run(service_address).await
}

fn with<T>(item: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Send + Sync + Clone,
{
    warp::any().map(move || item.clone())
}
