use std::collections::HashMap;

use crate::{ws, Client, Clients, EmojiMessage, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, ws::Message, Reply};

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    message: String,
    emojis: Vec<String>,
}

pub async fn publish_handler(event: Event, clients: Clients) -> Result<impl Reply> {
    let event = serde_json::to_string(&event).unwrap();
    clients.read().await.iter().for_each(|(_, connected_user)| {
        for (_, client) in connected_user {
            if let Some(sender) = &client.sender {
                let _ = sender.send(Ok(Message::text(&event)));
            }
        }
    });

    Ok(StatusCode::OK)
}

pub async fn register_handler(
    headers: warp::http::HeaderMap,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<EmojiMessage>,
) -> Result<impl Reply> {
    info!("Got registration call!");
    let identity = headers
        .get("X-SSO-EMAIL")
        .ok_or(warp::reject::not_found())?
        .as_bytes();
    let identity = String::from_utf8(identity.to_vec()).map_err(|_| warp::reject::not_found())?;

    debug!("Registering client for {}", identity);

    let guid = Uuid::new_v4().as_simple().to_string();

    register_client(guid.clone(), identity, clients, emoji_sender).await;
    Ok(json(&RegisterResponse {
        url: format!("ws://127.0.0.1:8000/ws/{}", guid),
    }))
}

async fn register_client(
    guid: String,
    identity: String,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<EmojiMessage>,
) {
    let mut clients = clients.write().await;

    if let Some(user_clients) = clients.get_mut(&identity) {
        user_clients.insert(
            guid,
            Client {
                sender: None,
                emoji_sender,
            },
        );
    } else {
        let mut user_clients = HashMap::new();
        user_clients.insert(
            guid,
            Client {
                sender: None,
                emoji_sender,
            },
        );
        clients.insert(identity, user_clients);
    }
}

pub async fn ws_handler(
    headers: warp::http::HeaderMap,
    ws: warp::ws::Ws,
    guid: String,
    clients: Clients,
) -> Result<impl Reply> {
    info!("Got websocket call!");
    let identity = headers
        .get("X-SSO-EMAIL")
        .ok_or(warp::reject::not_found())?
        .as_bytes();
    let identity = String::from_utf8(identity.to_vec()).map_err(|_| warp::reject::not_found())?;

    info!("Websocket upgrade for {identity}!");

    let client = clients
        .read()
        .await
        .get(&identity)
        .ok_or(warp::reject::not_found())?
        .get(&guid)
        .ok_or(warp::reject::not_found())?
        .clone();

    Ok(ws.on_upgrade(move |socket| ws::client_connection(socket, identity, guid, clients, client)))
}

pub async fn health_handler() -> Result<impl Reply> {
    Ok(StatusCode::OK)
}
