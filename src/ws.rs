use crate::{Client, IdentifiedIncomingMessage, Presentation, IncomingMessage, OutgoingUserMessage, OutgoingMessage, Presenter, User};
use futures::{FutureExt, StreamExt, stream::SplitStream};
use tokio::sync::mpsc::{self, UnboundedSender, UnboundedReceiver};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

async fn handle_presenter_messages(presenter: Presenter, presentation: Presentation, mut client_ws_rcv: SplitStream<WebSocket>, mut closer_rcv: UnboundedReceiver<()>) {
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
                        handle_msg(identity, msg, sender, client).await;
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
    };
}

async fn handle_user_messages(user: User, presentation: Presentation, mut client_ws_rcv: SplitStream<WebSocket>, mut closer_rcv: UnboundedReceiver<()>) {
    let guid = &user.guid;
    let identity = &user.identity;
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
                user.send_ignore_fail(OutgoingUserMessage::Disconnected(String::new()));
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
        warn!("{identity} - {guid} was already disconnected from {}", presentation.id)
    }
}

pub async fn new_connection<T: OutgoingMessage>(
    ws: WebSocket,
    presentation: Presentation,
    guid: String,
    user_message_sender: UnboundedSender<IdentifiedIncomingMessage<T>>,
) {
    // Take the web socket and split it into a sender and receiver. The sender and receiver here
    // are not directly connected. The sender sends messages to the client, and receiver receives
    // responses which may or may not be related to those messages.
    let (client_ws_sender, mut client_ws_rcv) = ws.split();

    // Create a channel to send messages to the client that is easier to pass around without polluting
    // the entire codebase with websocket types.
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    // Create an internal messaging channel to close the connection when we drop the client
    let (closer, mut closer_rcv) = mpsc::unbounded_channel::<()>();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    let is_presenter = presentation
        .presenters
        .contains_key(&guid);

    if is_presenter {
        // If they are the presenter, then we add them to the presenters data structure
        let presenter = if let Some(presenter) = presentation.presenters.get(&guid) {
            presenter.value().to_owned()
        } else {
            error!("A presenter could not upgrade their client because they have not registered");
            return;
        };

        // Add the channels to complete the connection
        presenter.sender = Some(client_sender.clone());
        presenter.closer = Some(closer);
        presentation.presenters.insert(guid.clone(), presenter);

        handle_presenter_messages(presenter, presentation, client_ws_rcv, closer_rcv).await;
    } else {
        // If they are a user, then we add them to the users data structure
        let user = if let Some(user) = presentation.users.get_by_guid(&guid) {
            user
        } else {
            warn!("{guid} could not upgrade their client because they have not registered");
            return;
        };

        // Add the channels to complete the connection
        user.sender = Some(client_sender.clone());
        user.closer = Some(closer);
        presentation.users.insert(user);

        // Send the initial presentation data including the current slide data
        let _ = client_sender.send(Ok(Message::text(OutgoingUserMessage::InitialPresentationData {
            title: presentation.get_title(),
            settings: presentation.slide_settings.read().await.clone()
        }.json())));
        
        handle_user_messages(user, presentation, client_ws_rcv, closer_rcv).await;
    }
}

async fn handle_msg(
    identity: &str,
    msg: Message,
    sender: UnboundedSender<IdentifiedIncomingMessage<impl OutgoingMessage>>,
    client: Client<impl OutgoingMessage>,
) {
    info!("received message from {}: {:?}", identity, msg);

    let message = match msg.to_str().map(serde_json::from_str::<IncomingMessage>) {
        Ok(Ok(m)) => m,
        _ => {
            error!("{identity} sent an invalid message");
            return;
        }
    };

    let _ = sender.send(IdentifiedIncomingMessage {
        client,
        message,
    });
}
