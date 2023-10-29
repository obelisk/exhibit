use crate::{
    ws, ClientJoinPresentationData, IdentifiedIncomingMessage,
    Presentation, Presentations, Presenter, User,
};
use serde::Serialize;
use tokio::sync::mpsc::UnboundedSender;
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

    let presentation = presentations
        .get(&user_auth_data.presentation)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value();
    let presentation_id = &presentation.id;
    let identity = user_auth_data.claims.sub.as_str();

    let guid = if presentation.presenter_identity == identity {
        debug!("Registering presenter for [{presentation_id}]");
        let new_presenter = Presenter::new(identity.to_owned(), presentation_id.to_owned());
        let guid = new_presenter.guid.clone();
        presentation.presenters.insert(guid.clone(), new_presenter);

        guid
    } else {
        debug!(
            "Registering user [{}] for presentation [{}]",
            user_auth_data.claims.sub, user_auth_data.presentation
        );

        let new_user = User::new(identity.to_owned(), presentation_id.to_owned());
        let guid = new_user.guid.clone();
        presentation.users.insert(new_user);

        guid
    };

    info!("{identity} is preparing to upgrade connection in [{presentation_id}] with guid [{guid}]");

    Ok(json(&RegisterResponse {
        url: format!("/ws/{presentation_id}/{guid}"),
    }))
}

pub async fn ws_handler<T>(
    presentation_id: String,
    guid: String,
    ws: warp::ws::Ws,
    presentations: Presentations,
    user_message_sender: UnboundedSender<IdentifiedIncomingMessage<T>>,
) -> Result<impl Reply> {
    trace!("Got websocket call for presentation: {presentation_id}!");
    let presentation = presentations
        .get(&presentation_id)
        .ok_or(warp::reject::not_found())?;

    let presentation = presentation.value().to_owned();

    // Is there a registered user for this guid
    let is_user =  presentation.users.get_by_guid(&guid).map(|x| x.clone()).is_some();
    // Is there a registered presenter for this guid
    let is_presenter =  presentation
        .presenters
        .get(&guid)
        .map(|x| x.value().clone()).is_some();

    // If there is neither
    if !is_user && !is_presenter {
            warn!("Got websocket upgrade for [{presentation_id}] with guid [{guid}] but no client or presenter is registered");
            return Err(warp::reject::not_found());
    }

    Ok(ws
        .max_message_size(1024 * 4) // Set max message size to 4KiB
        .on_upgrade(move |socket| {
            ws::client_connection(
                socket,
                presentation,
                guid,
                user_message_sender,
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
