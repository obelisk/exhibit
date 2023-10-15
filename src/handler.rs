use crate::{
    ws, Client, ClientJoinPresentationData, Clients, IdentifiedUserMessage, JwtClaims,
    NewPresentationRequest, Presentation, Presentations, Presenters, Result,
};
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, Reply};

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

pub async fn join_jwt_handler(
    user_auth_data: ClientJoinPresentationData,
    presentations: Presentations,
) -> Result<impl Reply> {
    debug!("Got user joining call");
    let guid = Uuid::new_v4().as_simple().to_string();

    let presentation = presentations
        .get(&user_auth_data.presentation)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value();
    let presentation_id = &presentation.id;

    if presentation.presenter_identity == user_auth_data.claims.sub {
        debug!("Registering presenter {presentation_id}");
        register_presenter(
            guid.clone(),
            user_auth_data.claims.sub,
            presentation.clients.clone(),
            user_auth_data.presentation,
        )
        .await;
    } else {
        debug!(
            "Registering client [{}] via JWT for presentation [{}]",
            user_auth_data.claims.sub, user_auth_data.presentation
        );

        register_client(
            guid.clone(),
            user_auth_data.claims.sub,
            presentation.clients.clone(),
            user_auth_data.presentation,
        )
        .await;
    }

    Ok(json(&RegisterResponse {
        url: format!("/ws/{presentation_id}/{guid}"),
    }))
}

async fn register_client(guid: String, identity: String, clients: Clients, presentation: String) {
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

async fn register_presenter(
    guid: String,
    identity: String,
    presenters: Presenters,
    presentation: String,
) {
    presenters.insert(
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
    presentation: Presentation,
    presentations: Presentations,
) -> Result<impl Reply> {
    debug!("Registering presentation {}", presentation.id);

    if presentations.get(&presentation.id).is_some() {
        error!(
            "Refusing to register a new version of presentation: {}",
            presentation.id
        );
        return Err(warp::reject::reject());
    }
    presentations.insert(presentation.id.clone(), presentation);

    Ok(StatusCode::OK)
}
