use crate::{Client, IdentifiedUserMessage, Presentation, UserMessage};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub async fn client_connection(
    ws: WebSocket,
    presentation: Presentation,
    guid: String,
    mut client: Client,
    user_message_sender: UnboundedSender<IdentifiedUserMessage>,
    is_presenter: bool,
) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let identity = client.identity.clone();
    let presentation_id = &presentation.id;

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    // Upgrade the client with the sender
    client.sender = Some(client_sender);

    // TODO @obelisk: Come back and make this code better
    // Create strict scoping here to release the lock
    {
        let mut upgrade_client = if is_presenter {
            match presentation.presenters.get_mut(&guid) {
                Some(client) => client,
                None => {
                    error!(
                    "{identity} (as a presenter) could not upgrade their client because they have not registered"
                );
                    return;
                }
            }
        } else {
            match presentation.clients.get_mut(&guid) {
                Some(client) => client,
                None => {
                    error!(
                    "{identity} could not upgrade their client because they have not registered"
                );
                    return;
                }
            }
        };

        *upgrade_client = client.clone();
    }

    info!("{identity} has new client with {guid} for {presentation_id}");

    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!(
                    "error receiving ws message for id: {}): {}",
                    guid.clone(),
                    e
                );
                break;
            }
        };
        client_msg(
            &identity,
            &guid,
            msg,
            user_message_sender.clone(),
            client.clone(),
        )
        .await;
    }

    if let Some(_) = presentation.clients.remove(&guid) {
        info!("{identity} - {guid} disconnected from {presentation_id}");
    } else {
        error!("{identity} - {guid} was already disconnected from {presentation_id}")
    }
}

async fn client_msg(
    identity: &str,
    guid: &str,
    msg: Message,
    sender: UnboundedSender<IdentifiedUserMessage>,
    client: Client,
) {
    info!("received message from {}: {:?}", identity, msg);

    let user_message = match msg.to_str().map(|x| serde_json::from_str::<UserMessage>(x)) {
        Ok(Ok(m)) => m,
        _ => {
            error!("{identity} sent an invalid message");
            return;
        }
    };

    let _ = sender.send(IdentifiedUserMessage {
        client,
        user_message,
    });
}
