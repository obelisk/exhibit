use crate::{Client, IdentifiedIncomingMessage, Presentation, IncomingMessage, OutgoingUserMessage};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub async fn client_connection(
    ws: WebSocket,
    presentation: Presentation,
    guid: String,
    mut client: Client,
    user_message_sender: UnboundedSender<IdentifiedIncomingMessage>,
    is_presenter: bool,
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

    let identity = client.identity.clone();
    let presentation_id = &presentation.id;

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    // Upgrade the client with the sender
    client.sender = Some(client_sender.clone());
    client.closer = Some(closer);

    if is_presenter {
        if  presentation.presenters.contains_key(&guid) {
            presentation.presenters.insert(guid.clone(), client.clone());
        } else {
            warn!(
                "{identity} (as a presenter) could not upgrade their client because they have not registered"
            );
            return;
        }
    } else {
        if presentation.clients.get_by_guid(&guid).is_some() {
            presentation.clients.insert(client.clone());
        } else {
            warn!(
                "{identity} could not upgrade their client because they have not registered"
            );
            return;
        }
    }

    info!("{identity} has new client with {guid} for {presentation_id}");

    // Send the initial presentation data including the current slide data
    let _ = client_sender.send(Ok(Message::text(OutgoingUserMessage::InitialPresentationData {
        title: presentation.get_title(),
        settings: presentation.slide_settings.read().await.clone()
    }.json())));

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
        
                        // Process the message received
                        client_msg(
                            &identity,
                            &guid,
                            msg,
                            user_message_sender.clone(),
                            client.clone(),
                        ).await
                    },
                    Some(Err(e)) => {
                        error!(
                            "error receiving ws message for id: {}): {}",
                            guid.clone(),
                            e
                        );
                        break;
                    }
                    None => {
                        error!("Got None from the client_ws_rcv for {identity}. Going to close connection.");
                        break;
                    }
                };
            }
            _ = closer_rcv.recv() => {
                info!("{identity} - is switching to a new device for {presentation_id}");
                // Internal request to close the connection
                let _ = client_sender.send(Ok(Message::text(OutgoingUserMessage::Disconnected(String::new()).json())));
                let _ = client_sender.send(Ok(Message::close()));
                break;
            }
        }
    };

    // If we are no longer receiving sensible messages or we're closing the connection
    // make sure they are cleaned up from the clients struture.
    //
    // If we initiated the close, the user will already be disconnected.
    if presentation.clients.remove(&client) {
        info!("{identity} - {guid} disconnected from {presentation_id}");
    } else {
        warn!("{identity} - {guid} was already disconnected from {presentation_id}")
    }
}

async fn client_msg(
    identity: &str,
    _guid: &str,
    msg: Message,
    sender: UnboundedSender<IdentifiedIncomingMessage>,
    client: Client,
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
