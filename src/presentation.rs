use std::sync::Arc;

use dashmap::DashMap;
use jsonwebtoken::DecodingKey;
use tokio::sync::RwLock;

use crate::{Clients, Presenters, ratelimiting::{Ratelimiter, time::TimeLimiter}, SlideSettings};

#[derive(Clone)]
pub struct PresentationData {
    /// The title of the presentation. Typically this is only
    /// sent to clients when they first connect.
    pub title: String,
    /// Created poles in the presentation. The key is the name of the pole.
    /// the value is map from user identity to what their answer was.
    pub poles: Arc<DashMap<String, DashMap<String, u32>>>,
}

impl PresentationData {
    pub fn new(title: String) -> Self {
        Self {
            title,
            poles: Arc::new(DashMap::new()),
        }
    }
}

#[derive(Clone)]
pub struct Presentation {
    pub id: String,
    pub presenter_identity: String,
    pub clients: Clients,
    pub presenters: Presenters,
    pub authentication_key: DecodingKey,
    pub ratelimiter: Arc<Ratelimiter>,
    pub slide_settings: Arc<RwLock<Option<SlideSettings>>>,
    pub encrypted: bool,
    pub presentation_data: PresentationData,
}

impl Presentation {
    pub fn new(
        presentation_id: String,
        presenter_identity: String,
        encrypted: bool,
        authentication_key: DecodingKey,
        title: String,
    ) -> Self {
        // Create a default 15s ratelimiter
        let ratelimiter = Arc::new(Ratelimiter::new());
        ratelimiter.add_ratelimit("15s".to_string(), Arc::new(TimeLimiter::new(15)));

        Self {
            id: presentation_id,
            presenter_identity,
            clients: Clients::new(),
            presenters: Arc::new(DashMap::new()),
            authentication_key,
            ratelimiter,
            slide_settings: Arc::new(None.into()),
            encrypted,
            presentation_data: PresentationData::new(title),
        }
    }
}
