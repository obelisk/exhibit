#[macro_use]
extern crate log;

use exhibit::{handler, processor, Clients, ConfigurationMessage, EmojiMessage};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let presenters = Arc::new(RwLock::new(HashMap::new()));

    let health_route = warp::path!("health").and_then(handler::health_handler);

    let (client_emoji_sender, client_emoji_receiver) = mpsc::unbounded_channel::<EmojiMessage>();
    let (configuration_sender, configuration_receiver) =
        mpsc::unbounded_channel::<ConfigurationMessage>();

    // Client routes
    let register_routes = warp::path("register")
        .and(warp::get())
        .and(warp::header::headers_cloned())
        .and(with(clients.clone()))
        .and(with(client_emoji_sender.clone()))
        .and_then(handler::register_handler);

    let client_ws_route = warp::path("ws")
        .and(warp::header::headers_cloned())
        .and(warp::ws())
        .and(warp::path::param())
        .and(with(clients.clone()))
        .and_then(handler::client_ws_handler);

    let client_spa = warp::path("/").and(warp::fs::file("web/client.html"));

    let client_routes = health_route
        .or(register_routes)
        .or(client_ws_route)
        .or(client_spa);

    // Admin/Presenter routes
    let update = warp::path!("update")
        .and(warp::post())
        .and(warp::body::json())
        .and(with(clients.clone()))
        .and(with(configuration_sender.clone()))
        .and_then(handler::update_handler);

    let presenter_emoji_stream = warp::path("emoji_stream")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with(presenters.clone()))
        .and_then(handler::presenter_ws_handler);

    let presenter_spa = warp::path("present").and(warp::fs::file("web/present.html"));

    let presenter_routes = update.or(presenter_emoji_stream).or(presenter_spa);

    tokio::task::spawn(async move {
        processor::handle_sent_emojis(client_emoji_receiver, configuration_receiver, presenters)
            .await;

        error!("A receiver was dropped?");
    });

    tokio::join!(
        warp::serve(client_routes).run(([0, 0, 0, 0], 8000)),
        warp::serve(presenter_routes).run(([0, 0, 0, 0], 8001))
    );
}

fn with<T>(users: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Send + Sync + Clone,
{
    warp::any().map(move || users.clone())
}
