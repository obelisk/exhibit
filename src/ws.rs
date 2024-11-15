use crate::{processor, IncomingMessage, OutgoingUserMessage, Presentation, Presenter, User};
use futures::{stream::SplitStream, FutureExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

async fn handle_presenter_messages(
    presenter: Presenter,
    presentation: Presentation,
    mut client_ws_rcv: SplitStream<WebSocket>,
    mut closer_rcv: UnboundedReceiver<()>,
) {
    let guid = &presenter.guid;
    let identity = &presenter.identity;
    // Handle all messages from the client as well as if we indend on closing the connection
    // from the server. This happens when the client is removed from the list of active clients
    loop {
        tokio::select! {
            result = client_ws_rcv.next() => {
                match result {
                    Some(Ok(msg)) => {
                        // If the connection is closed/to be closed then break
                        if msg.is_close() {
                            break;
                        }
                        let message = match msg.to_str().map(serde_json::from_str::<IncomingMessage>) {
                            Ok(Ok(m)) => m,
                            Ok(Err(e)) => {
                                error!("A presenter sent an invalid message: {e}");
                                continue;
                            }
                            Err(_) => {
                                error!("A preesnter sent a message which wasn't text!");
                                continue;
                            }
                        };
                        match message {
                            IncomingMessage::Presenter(presenter_message) => processor::handle_presenter_message_types(presenter_message, presenter.clone(), presentation.clone()).await,
                            _ => {
                                warn!("{identity} sent a valid message but it was not a presenter message");
                                continue;
                            }
                        }

                    },
                    Some(Err(e)) => {
                        error!("error receiving ws message for id: [{guid}]: {e}");
                        break;
                    }
                    None => {
                        error!("Got None from the client_ws_rcv for {identity}. Going to close connection.");
                        break;
                    }
                };
            }
            _ = closer_rcv.recv() => {
                info!("{identity} - is switching to a new device for {}", presentation.id);
                // Inform the presenter the connection is being close
                //presenter.send_ignore_fail(OutgoingUserMessage::Disconnected(String::new()));
                break;
            }
        }
    }
    warn!("Done handling presenter messages for: [{identity}] on [{guid}]");
}

async fn handle_user_messages(
    user: User,
    presentation: Presentation,
    mut client_ws_rcv: SplitStream<WebSocket>,
    mut closer_rcv: UnboundedReceiver<()>,
) {
    let guid = &user.guid;
    let identity = &user.identity;
    debug!("Handling user messages for [{identity}] on guid [{guid}]");
    // Handle all messages from the client as well as if we indend on closing the connection
    // from the server. This happens when the client is removed from the list of active clients
    loop {
        tokio::select! {
            result = client_ws_rcv.next() => {
                match result {
                    Some(Ok(msg)) => {
                        // If the connection is closed/to be closed then break
                        if msg.is_close() {
                            break;
                        }

                        let message = match msg.to_str().map(serde_json::from_str::<IncomingMessage>) {
                            Ok(Ok(m)) => m,
                            _ => {
                                error!("{identity} sent an invalid message");
                                continue;
                            }
                        };
                        match message {
                            IncomingMessage::User(user_message) => processor::handle_user_message_types(user_message, user.clone(), presentation.clone()).await,
                            _ => {
                                error!("{identity} sent an invalid message");
                                continue;
                            }
                        }

                    },
                    Some(Err(e)) => {
                        error!("error receiving ws message for id: [{guid}]: {e}");
                        break;
                    }
                    None => {
                        error!("Got None from the client_ws_rcv for {identity}. Going to close connection.");
                        break;
                    }
                };
            }
            _ = closer_rcv.recv() => {
                info!("{identity} - is switching to a new device for {}", presentation.id);
                // Internal request to close the connection
                user.send_ignore_fail(OutgoingUserMessage::Disconnect(String::new()));
                break;
            }
        }
    }

    // If we are no longer receiving sensible messages or we're closing the connection
    // make sure they are cleaned up from the clients struture.
    //
    // If we initiated the close, the user will already be disconnected.
    if presentation.users.remove(&user) {
        info!("{identity} - {guid} disconnected from {}", presentation.id);
    } else {
        warn!(
            "{identity} - {guid} was already disconnected from {}",
            presentation.id
        )
    }
}

pub async fn new_connection(ws: WebSocket, presentation: Presentation, guid: String) {
    // Take the web socket and split it into a sender and receiver. The sender and receiver here
    // are not directly connected. The sender sends messages to the client, and receiver receives
    // responses which may or may not be related to those messages.
    let (client_ws_sender, client_ws_rcv) = ws.split();

    // Create a channel to send messages to the client that is easier to pass around without polluting
    // the entire codebase with websocket types.
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    // Create an internal messaging channel to close the connection when we drop the client
    let (closer, closer_rcv) = mpsc::unbounded_channel::<()>();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    let is_presenter = presentation.presenters.contains_key(&guid);

    if is_presenter {
        // If they are the presenter, then we add them to the presenters data structure
        let mut presenter = if let Some(presenter) = presentation.presenters.get(&guid) {
            presenter.value().to_owned()
        } else {
            error!("A presenter could not upgrade their client because they have not registered");
            return;
        };

        // Add the channels to complete the connection
        presenter.sender = Some(client_sender.clone());
        presenter.closer = Some(closer);
        presentation
            .presenters
            .insert(guid.clone(), presenter.clone());

        handle_presenter_messages(presenter, presentation, client_ws_rcv, closer_rcv).await;

        warn!("A presenter connection has just closed!");
    } else {
        // If they are a user, then we add them to the users data structure
        let mut user = if let Some(user) = presentation.users.get_by_guid(&guid) {
            user
        } else {
            warn!("{guid} could not upgrade their client because they have not registered");
            return;
        };

        // Add the channels to complete the connection
        user.sender = Some(client_sender.clone());
        user.closer = Some(closer);
        presentation.users.insert(user.clone());

        // Send the initial presentation data including the current slide data
        let _ = client_sender.send(Ok(Message::text(
            OutgoingUserMessage::InitialPresentationData {
                title: presentation.get_title(),
                settings: presentation.slide_settings.read().await.clone(),
            }
            .json(),
        )));

        let identity = user.identity.clone();
        handle_user_messages(user, presentation, client_ws_rcv, closer_rcv).await;

        info!("User connection for [{identity}] has finished");
    }
}
