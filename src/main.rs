#[macro_use]
extern crate log;

use exhibit::{
    authentication::{parse_jwt_presentation_join, parse_jwt_presentation_new},
    config, handler, processor, IdentifiedUserMessage, Presentations,
};
use tokio::sync::mpsc::unbounded_channel;

use std::convert::Infallible;
use std::net::SocketAddr;
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Configure Exhibit
    let config_path = std::env::args()
        .nth(1)
        .expect("Please provide a configuration file path");
    let configuration = config::load_configuration(&config_path);

    // Probably the most important data structure in the whole application.
    // Stores all the presenters and clients for all active presentations
    let presentations = Presentations::new();

    let (user_message_sender, user_message_receiver) = unbounded_channel::<IdentifiedUserMessage>();

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
        .and(warp::body::bytes().and_then(move |provided_token| {
            parse_jwt_presentation_new(
                configuration.new_presentation_authorization_key.clone(),
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
            parse_jwt_presentation_join(provided_token, presentation_capture.clone())
        }))
        .and(with(presentations.clone()))
        .and(with(user_message_sender.clone()))
        .and_then(handler::join_jwt_handler);

    // SPAs
    let client_spa = warp::path::end().and(warp::fs::file("web/client.html"));
    let presenter_spa = warp::path("present").and(warp::fs::file("web/present.html"));

    let client_routes = health_route
        .or(join_route)
        .or(client_ws_route)
        .or(client_spa);

    // let presenter_emoji_stream = warp::path("emoji_stream")
    //     .and(warp::ws())
    //     .and(warp::path::param())
    //     .and(with(presenters.clone()))
    //     .and_then(handler::presenter_ws_handler);

    let presenter_routes = presenter_spa.or(new_presentation);
    //.or(presenter_emoji_stream)
    //.or(update);

    let presentations_clone = presentations.clone();
    tokio::task::spawn(async move {
        processor::handle_sent_messages(user_message_receiver, presentations_clone).await;

        panic!("User message receiver was dropped?");
    });

    let all_routes = client_routes.or(presenter_routes);

    let service_address: SocketAddr = configuration.service_address.parse().unwrap();

    warp::serve(all_routes).run(service_address).await
}

fn with<T>(item: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Send + Sync + Clone,
{
    warp::any().map(move || item.clone())
}
