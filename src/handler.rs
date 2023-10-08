use crate::{
    ws, Client, Clients, ConfigurationMessage, IdentifiedUserMessage, JwtClaims, Presenters, Result,
};
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
            clients.read().await.iter().for_each(|(_, client)| {
                if let Some(sender) = &client.sender {
                    let _ = sender.send(Ok(Message::text(&event)));
                }
            });
        }
    }

    Ok(StatusCode::OK)
}

pub async fn register_header_handler(
    header: String,
    headers: warp::http::HeaderMap,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<IdentifiedUserMessage>,
) -> Result<impl Reply> {
    info!("Got header registration call!");
    let identity = headers
        .get(header)
        .ok_or(warp::reject::not_found())?
        .as_bytes();
    let identity = String::from_utf8(identity.to_vec()).map_err(|_| warp::reject::not_found())?;

    debug!("Registering client for [{identity}]");

    let guid = Uuid::new_v4().as_simple().to_string();

    register_client(guid.clone(), identity, clients, emoji_sender).await;
    Ok(json(&RegisterResponse {
        url: format!("/ws/{}", guid),
    }))
}

pub async fn register_jwt_handler(
    token: JwtClaims,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<IdentifiedUserMessage>,
) -> Result<impl Reply> {
    debug!("Registering client via JWT for [{}]", token.sub);

    let guid = Uuid::new_v4().as_simple().to_string();

    register_client(guid.clone(), token.sub, clients, emoji_sender).await;
    Ok(json(&RegisterResponse {
        url: format!("/ws/{}", guid),
    }))
}

async fn register_client(
    guid: String,
    identity: String,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<IdentifiedUserMessage>,
) {
    let mut clients = clients.write().await;

    clients.insert(
        guid,
        Client {
            sender: None,
            emoji_sender,
            identity,
        },
    );
}

pub async fn client_ws_handler(
    ws: warp::ws::Ws,
    guid: String,
    clients: Clients,
) -> Result<impl Reply> {
    info!("Got websocket call!");
    let client = clients
        .read()
        .await
        .get(&guid)
        .ok_or(warp::reject::not_found())?
        .clone();

    info!("Websocket upgrade for {}!", client.identity);

    Ok(ws
        .max_message_size(1024 * 2)
        .on_upgrade(move |socket| ws::client_connection(socket, guid, clients, client)))
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
