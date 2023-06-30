use std::collections::HashMap;

use crate::{
    ws, Client, Clients, ConfigurationMessage, EmojiMessage, Presenters, RawToken, Result, Token,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedSender};
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, ws::Message, Reply};

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

type HmacSha256 = Hmac<Sha256>;

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

pub async fn register_token_handler(
    token: RawToken,
    clients: Clients,
    emoji_sender: mpsc::UnboundedSender<EmojiMessage>,
) -> Result<impl Reply> {
    info!("Got token registration call!");
    // Decode the token base64
    let identity: Token = match base64::decode(&token.token).map(|x| serde_json::from_slice(&x)) {
        Ok(Ok(identity)) => identity,
        _ => {
            error!("Unparsable token received");
            return Err(warp::reject::not_found());
        }
    };

    let mut mac = HmacSha256::new_from_slice(b"password").expect("HMAC can take key of any size");

    mac.update(&identity.email.as_bytes());

    let verification_bytes = match base64::decode(&identity.mac) {
        Ok(verification_bytes) => verification_bytes,
        _ => {
            error!("Unparsable MAC for {}", identity.email);
            return Err(warp::reject::not_found());
        }
    };

    if let Err(_) = mac.verify_slice(&verification_bytes[..]) {
        error!("Invalid token for {}", identity.email);
        return Err(warp::reject::not_found());
    }

    debug!("Registering client for {}", identity.email);

    let guid = Uuid::new_v4().as_simple().to_string();

    register_client(guid.clone(), identity.email, clients, emoji_sender).await;
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

    Ok(ws.on_upgrade(move |socket| ws::client_connection(socket, guid, clients, client)))
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
