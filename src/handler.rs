use crate::{
    ws, Client, ClientJoinPresentationData, IdentifiedIncomingMessage,
    Presentation, Presentations,
};
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;
use warp::{http::StatusCode, reply::json, Reply, reject::Rejection};


type Result<T> = std::result::Result<T, Rejection>;

#[derive(Serialize, Debug)]
pub struct RegisterResponse {
    url: String,
}

pub async fn join_handler(
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
    let identity = user_auth_data.claims.sub.as_str();

    let new_client = Client {
        sender: None,
        closer: None,
        identity: identity.to_owned(),
        guid: guid.clone(),
        presentation: user_auth_data.presentation.clone(),
    };

    if presentation.presenter_identity == identity {
        debug!("Registering presenter for [{presentation_id}]");
        presentation.presenters.insert(guid.clone(), new_client);
    } else {
        debug!(
            "Registering client [{}] for presentation [{}]",
            user_auth_data.claims.sub, user_auth_data.presentation
        );
        presentation.clients.insert(new_client);
    }

    info!("{identity} is preparing to upgrade connection in [{presentation_id}] with guid [{guid}]");

    Ok(json(&RegisterResponse {
        url: format!("/ws/{presentation_id}/{guid}"),
    }))
}

pub async fn ws_handler(
    presentation_id: String,
    guid: String,
    ws: warp::ws::Ws,
    presentations: Presentations,
    user_message_sender: UnboundedSender<IdentifiedIncomingMessage>,
) -> Result<impl Reply> {
    trace!("Got websocket call for presentation: {presentation_id}!");
    let presentation = presentations
        .get(&presentation_id)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value().to_owned();

    // There is no registered client or presenter for this websocket
    let (client, is_presenter) = match (
        presentation.clients.get_by_guid(&guid).map(|x| x.clone()),
        presentation
            .presenters
            .get(&guid)
            .map(|x| x.value().clone()),
    ) {
        (None, None) => {
            warn!("Got websocket upgrade for [{presentation_id}] with guid [{guid}] but no client or presenter is registered");
            return Err(warp::reject::not_found())
        },

        (Some(x), _) => (x, false),
        (_, Some(x)) => (x, true),
    };

    Ok(ws
        .max_message_size(1024 * 4) // Set max message size to 4KiB
        .on_upgrade(move |socket| {
            ws::client_connection(
                socket,
                presentation,
                guid,
                client,
                user_message_sender,
                is_presenter,
            )
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
