use crate::{
    ws, Client, Clients, ConfigurationMessage, IdentifiedUserMessage, JwtClaims, Presentation,
    Presentations, Presenters, Result,
};
use dashmap::DashMap;
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
            clients.iter().for_each(|item| {
                let client = item.value();
                if let Some(sender) = &client.sender {
                    let _ = sender.send(Ok(Message::text(&event)));
                }
            });
        }
    }

    Ok(StatusCode::OK)
}

pub async fn join_jwt_handler(
    token: JwtClaims,
    presentations: Presentations,
    user_message_sender: UnboundedSender<IdentifiedUserMessage>,
) -> Result<impl Reply> {
    debug!(
        "Registering client [{}] via JWT for presentation [{}]",
        token.sub, token.kid
    );

    let guid = Uuid::new_v4().as_simple().to_string();

    let presentation = presentations
        .get(&token.kid)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value();
    let presentation_id = &presentation.id;

    register_client(
        guid.clone(),
        token.sub,
        presentation.clients.clone(),
        user_message_sender,
    )
    .await;
    Ok(json(&RegisterResponse {
        url: format!("/ws/{presentation_id}/{guid}"),
    }))
}

async fn register_client(
    guid: String,
    identity: String,
    clients: Clients,
    user_message_sender: UnboundedSender<IdentifiedUserMessage>,
) {
    clients.insert(
        guid,
        Client {
            sender: None,
            identity,
        },
    );
}

pub async fn client_ws_handler(
    presentation_id: String,
    guid: String,
    ws: warp::ws::Ws,
    presentations: Presentations,
    user_message_sender: UnboundedSender<IdentifiedUserMessage>,
) -> Result<impl Reply> {
    trace!("Got websocket call for presentation: {presentation_id}!");
    let presentation = presentations
        .get(&presentation_id)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value().to_owned();
    let clients = presentation.clients.clone();
    let client = clients.get(&guid).ok_or(warp::reject::not_found())?.clone();

    info!(
        "Websocket upgrade for {} in {presentation_id}!",
        client.identity
    );

    Ok(ws.max_message_size(1024 * 2).on_upgrade(move |socket| {
        ws::client_connection(socket, presentation, guid, client, user_message_sender)
    }))
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

pub async fn new_presentation_hander(
    token: JwtClaims,
    presentations: DashMap<String, Presentation>,
) -> Result<impl Reply> {
    let presentation = Presentation::new(token.sub);
    presentations.insert(presentation.id.clone(), presentation);

    Ok(StatusCode::OK)
}
