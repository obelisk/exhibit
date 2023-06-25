#[macro_use]
extern crate log;

use exhibit::{handler, processor, Clients, ConfigurationMessage, EmojiMessage, SlideSettings};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

    let health_route = warp::path!("health").and_then(handler::health_handler);

    let (emoji_sender, mut emoji_receiver) = mpsc::unbounded_channel::<EmojiMessage>();
    let (configuration_sender, mut configuration_receiver) =
        mpsc::unbounded_channel::<ConfigurationMessage>();

    let client_spa = warp::path("client").and(warp::fs::file("client/client.html"));

    let register_routes = warp::path("register")
        .and(warp::get())
        .and(warp::header::headers_cloned())
        .and(with_clients(clients.clone()))
        .and(with_sender(emoji_sender.clone()))
        .and_then(handler::register_handler);

    let update = warp::path!("update")
        .and(warp::body::json())
        .and(with_clients(clients.clone()))
        .and(with_sender(configuration_sender.clone()))
        .and_then(handler::update_handler);

    let ws_route = warp::path("ws")
        .and(warp::header::headers_cloned())
        .and(warp::ws())
        .and(warp::path::param())
        .and(with_clients(clients.clone()))
        .and_then(handler::ws_handler);

    let routes = health_route.or(register_routes).or(ws_route).or(client_spa);

    let admin_routes = update;

    tokio::task::spawn(async move {
        processor::handle_sent_emojis(emoji_receiver, configuration_receiver).await;

        error!("A receiver was dropped?");
    });

    tokio::join!(
        warp::serve(routes).run(([127, 0, 0, 1], 8000)),
        warp::serve(admin_routes).run(([127, 0, 0, 1], 8001))
    );
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn with_sender<T>(
    sender: mpsc::UnboundedSender<T>,
) -> impl Filter<Extract = (mpsc::UnboundedSender<T>,), Error = Infallible> + Clone
where
    T: Send + Sync,
{
    warp::any().map(move || sender.clone())
}
