use crate::{Client, Clients, IdentifiedUserMessage, Presenter, Presenters, UserMessage};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub async fn client_connection(ws: WebSocket, guid: String, clients: Clients, mut client: Client) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let emoji_sender = client.emoji_sender.clone();
    let identity = client.identity.clone();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    {
        client.sender = Some(client_sender);
        let mut clients = clients.write().await;

        let user_client = match clients.get_mut(&guid) {
            Some(client) => client,
            None => {
                error!(
                    "{identity} could not upgrade their client because they have not registered"
                );
                return;
            }
        };

        *user_client = client;
    }

    info!("{identity} has new client with {guid}");

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
        client_msg(&identity, &guid, msg, emoji_sender.clone(), clients.clone()).await;
    }

    if let Some(_) = clients.write().await.remove(&guid) {
        info!("{identity} - {guid} disconnected");
    } else {
        error!("{identity} - {guid} was already disconnected")
    }
}

async fn client_msg(
    identity: &str,
    guid: &str,
    msg: Message,
    sender: UnboundedSender<IdentifiedUserMessage>,
    clients: Clients,
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
        identity: identity.to_string(),
        guid_identifier: guid.to_string(),
        clients,
        user_message,
    });
}

pub async fn presenter_connection(ws: WebSocket, guid: String, presenters: Presenters) {
    let (presenter_ws_sender, mut presenter_ws_rcv) = ws.split();
    let (presenter_sender, presenter_rcv) = mpsc::unbounded_channel();

    presenters.write().await.insert(
        guid.clone(),
        Presenter {
            sender: presenter_sender,
        },
    );

    let presenter_rcv = UnboundedReceiverStream::new(presenter_rcv);
    tokio::task::spawn(presenter_rcv.forward(presenter_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    while let Some(_) = presenter_ws_rcv.next().await {}

    if let Some(_) = presenters.write().await.remove(&guid) {
        info!("Presenter {guid} - disconnected");
    } else {
        error!("Presenter {guid} - was already disconnected")
    }
}
