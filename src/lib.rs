#[macro_use]
extern crate log;

pub mod authentication;
pub mod config;
pub mod handler;
pub mod messaging;
pub mod processor;
pub mod presentation;
pub mod ratelimiting;
pub mod ws;

use std::sync::Arc;

pub use presentation::Presentation;
pub use messaging::*;

use dashmap::{DashMap, mapref::multiple::RefMulti};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;
use warp::filters::ws::Message;

pub type User = Client<OutgoingUserMessage>;
pub type Presenter = Client<OutgoingPresenterMessage>;
pub type Presenters = Arc<DashMap<String, Presenter>>;
pub type Presentations = Arc<DashMap<String, Presentation>>;

#[derive(Debug, Clone)]
pub struct Client<T> where T: Clone {
    pub sender: Option<mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>>,
    pub closer: Option<mpsc::UnboundedSender<()>>,
    pub identity: String,
    pub guid: String,
    pub presentation: String,
    _phantom: std::marker::PhantomData<T>,
}

impl Client<OutgoingPresenterMessage> {
    pub fn new(identity: String, presentation: String) -> Self {
        Self {
            sender: None,
            closer: None,
            identity,
            guid: Uuid::new_v4().as_simple().to_string(),
            presentation,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn close(&mut self) {
        if let Some(sender) = self.closer.clone() {
            let _ = sender.send(());
            self.sender = None;
            self.closer = None;
        }
    }

    pub fn send_ignore_fail(&self, message: OutgoingPresenterMessage) {
        if let Some(ref sender) = self.sender {
            let _ = sender.send(Ok(Message::text(message.json())));
        }
    }
}

impl Client<OutgoingUserMessage> {
    pub fn new(identity: String, presentation: String) -> Self {
        Self {
            sender: None,
            closer: None,
            identity,
            guid: Uuid::new_v4().as_simple().to_string(),
            presentation,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn close(&mut self) {
        if let Some(sender) = self.closer.clone() {
            let _ = sender.send(());
            self.sender = None;
            self.closer = None;
        }
    }

    pub fn send_ignore_fail(&self, message: OutgoingUserMessage) {
        if let Some(ref sender) = self.sender {
            let _ = sender.send(Ok(Message::text(message.json())));
        }
    }
}

#[derive(Debug, Clone)]
pub struct Users {
    /// Maps the identifier provided by the authentication
    /// layer to the current websocket guid
    client_mapping: Arc<DashMap<String, String>>,
    /// Maps the guid provided by the client to the client connection
    guid_mapping: Arc<DashMap<String, User>>,
}

impl Users {
    pub fn new() -> Self {
        Self {
            client_mapping: Arc::new(DashMap::new()),
            guid_mapping: Arc::new(DashMap::new()),
        }
    }

    pub fn get_by_guid(&self, guid: &str) -> Option<User> {
        let guid_mapping = self.guid_mapping.get(guid)?;
        //let client = self.client_connections.get(guid_mapping.as_str())?;
        Some(guid_mapping.value().clone())
    }

    pub fn insert(&self, client: User) {
        debug!("inserting client with guid: {}", client.guid);
        // Clear the old session if it exists first
        if let Some(old_guid) = self.client_mapping.remove(client.identity.as_str()).map(|x| x.1) {
            debug!("There exists a previous connection for this client: [{}]. Closing it.", old_guid);
            if let Some(mut client) = self.guid_mapping.remove(&old_guid) {
                client.1.close();
            }
        }

        self.guid_mapping.insert(client.guid.clone(), client.clone());
        self.client_mapping.insert(client.identity.clone(), client.guid.clone());
    }

    pub fn remove(&self, client: &User) -> bool {
        debug!("Removing client with guid: {}", client.guid);
        self.client_mapping.remove(&client.identity);
        self.guid_mapping.remove(&client.guid).is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = RefMulti<String, User>> {
        self.guid_mapping.iter()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlideSettings {
    pub message: String,
    pub emojis: Vec<String>,
}

impl std::fmt::Display for SlideSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} - {}", self.emojis, self.message)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // Contains the user's identifying information
    pub pid: String, // Presentation ID, should always match the kid in header to be valid
    pub exp: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientJoinPresentationData {
    pub presentation: String,
    pub claims: JwtClaims,
}

