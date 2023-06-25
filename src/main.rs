#[macro_use]
extern crate log;

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use warp::{ws::Message, Filter, Rejection};

mod handler;
mod ws;

type Result<T> = std::result::Result<T, Rejection>;

// A user can be connected on multiple devices so we have a hashmap
// linking their identity to another hashmap of their connected
// devices
type Clients = Arc<RwLock<HashMap<String, HashMap<String, Client>>>>;

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub emoji_sender: mpsc::UnboundedSender<EmojiMessage>,
}

pub struct EmojiMessage {
    pub identity: String,
    pub emoji: String,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

    let health_route = warp::path!("health").and_then(handler::health_handler);

    let (emoji_sender, mut emoji_receiver) = mpsc::unbounded_channel::<EmojiMessage>();

    let register = warp::path("register");
    let register_routes = register
        .and(warp::get())
        .and(warp::header::headers_cloned())
        .and(with_clients(clients.clone()))
        .and(with_emoji_sender(emoji_sender.clone()))
        .and_then(handler::register_handler);

    let publish = warp::path!("publish")
        .and(warp::body::json())
        .and(with_clients(clients.clone()))
        .and_then(handler::publish_handler);

    let ws_route = warp::path("ws")
        .and(warp::header::headers_cloned())
        .and(warp::ws())
        .and(warp::path::param())
        .and(with_clients(clients.clone()))
        .and_then(handler::ws_handler);

    let routes = health_route
        .or(register_routes)
        .or(ws_route)
        .or(publish)
        .with(warp::cors().allow_any_origin());

    tokio::task::spawn(async move {
        while let Some(emoji_message) = emoji_receiver.recv().await {
            println!("{} sent {}", emoji_message.identity, emoji_message.emoji);
        }

        error!("Emoji receiver channel closed!");
    });

    warp::serve(routes).run(([127, 0, 0, 1], 8000)).await;
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn with_emoji_sender<T>(
    emoji_sender: mpsc::UnboundedSender<T>,
) -> impl Filter<Extract = (mpsc::UnboundedSender<T>,), Error = Infallible> + Clone
where
    T: Send + Sync,
{
    warp::any().map(move || emoji_sender.clone())
}
