use crate::{Client, Clients, EmojiMessage, Presenter, Presenters, UserMessage};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub async fn client_connection(
    ws: WebSocket,
    identity: String,
    guid: String,
    clients: Clients,
    mut client: Client,
) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();
    let emoji_sender = client.emoji_sender.clone();

    let client_rcv = UnboundedReceiverStream::new(client_rcv);
    tokio::task::spawn(client_rcv.forward(client_ws_sender).map(|result| {
        if let Err(e) = result {
            error!("error sending websocket msg: {}", e);
        }
    }));

    {
        client.sender = Some(client_sender);
        let mut clients = clients.write().await;

        let user_client = match clients.get_mut(&identity) {
            Some(ucs) => ucs.get_mut(&guid),
            None => {
                error!(
                    "{identity} could not upgrade their client because they have not registered"
                );
                return;
            }
        };

        match user_client {
            Some(c) => *c = client,
            None => {
                error!("{identity} could not upgrade client {guid} because it was not registered");
                return;
            }
        };
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
        client_msg(&identity, msg, emoji_sender.clone()).await;
    }

    if let Some(ucs) = clients.write().await.get_mut(&identity) {
        ucs.remove(&guid);
        info!("{identity} - {guid} disconnected");
    } else {
        error!("{identity} - {guid} was already disconnected")
    }
}

async fn client_msg(identity: &str, msg: Message, sender: UnboundedSender<EmojiMessage>) {
    info!("received message from {}: {:?}", identity, msg);

    let msg = match msg.to_str().map(|x| serde_json::from_str::<UserMessage>(x)) {
        Ok(Ok(m)) => m,
        _ => {
            error!("{identity} sent an invalid message");
            return;
        }
    };

    match msg {
        UserMessage::Emoji { slide, emoji } => {
            let _ = sender.send(EmojiMessage {
                identity: identity.to_string(),
                slide,
                emoji,
            });
        }
    }
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
