use crate::{
    ws, Client, Clients, IdentifiedUserMessage, JwtClaims, Presentation, Presentations, Presenters,
    Result,
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
        token.kid,
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
    presentation: String,
) {
    clients.insert(
        guid.clone(),
        Client {
            sender: None,
            identity,
            guid,
            presentation,
        },
    );
}

pub async fn ws_handler(
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

    // There is no registered client or presenter for this websocket
    let client = match (
        presentation.clients.get(&guid).map(|x| x.value().clone()),
        presentation
            .presenters
            .get(&guid)
            .map(|x| x.value().clone()),
    ) {
        (None, None) => return Err(warp::reject::not_found()),
        // It should never occur that it's both but if it is, assume client
        (Some(x), _) | (_, Some(x)) => x,
    };

    Ok(ws
        .max_message_size(1024 * 4) // Set max message size to 4KiB
        .on_upgrade(move |socket| {
            ws::client_connection(socket, presentation, guid, client, user_message_sender)
        }))
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
