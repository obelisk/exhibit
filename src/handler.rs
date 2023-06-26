use std::collections::HashMap;

use crate::{ws, Client, Clients, ConfigurationMessage, EmojiMessage, Presenters, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, ws::Message, Reply};

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    message: String,
    slide: u64,
    emojis: Vec<String>,
}

pub async fn update_handler(
    update: ConfigurationMessage,
    clients: Clients,
    configuration_sender: UnboundedSender<ConfigurationMessage>,
) -> Result<impl Reply> {
    let _ = configuration_sender.send(update.clone());

    match update {
        // A new slide is bring displayed
        ConfigurationMessage::NewSlide {
            slide,
            slide_settings,
        } => {
            let event = Event {
                message: slide_settings.message,
                slide,
                emojis: slide_settings.emojis,
            };
            let event = serde_json::to_string(&event).unwrap();
            clients.read().await.iter().for_each(|(_, connected_user)| {
                for (_, client) in connected_user {
                    if let Some(sender) = &client.sender {
                        let _ = sender.send(Ok(Message::text(&event)));
                    }
                }
            });
        }
    }

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
        url: format!("/ws/{}", guid),
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

pub async fn client_ws_handler(
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

pub async fn presenter_ws_handler(
    ws: warp::ws::Ws,
    guid: String,
    presenters: Presenters,
) -> Result<impl Reply> {
    info!("New websocket for emoji stream");

    Ok(ws.on_upgrade(move |socket| ws::presenter_connection(socket, guid, presenters)))
}

pub async fn health_handler() -> Result<impl Reply> {
    Ok(StatusCode::OK)
}
