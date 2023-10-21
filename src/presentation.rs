use std::sync::Arc;

use dashmap::DashMap;
use jsonwebtoken::DecodingKey;
use tokio::sync::RwLock;

use crate::{Clients, Presenters, ratelimiting::{Ratelimiter, time::TimeLimiter}, SlideSettings};

#[derive(Clone)]
pub struct PresentationData {
    pub title: String,
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
        // Create a default 10s ratelimiter
        let ratelimiter = Arc::new(Ratelimiter::new());
        ratelimiter.add_ratelimit("15s".to_string(), Arc::new(TimeLimiter::new(15)));

        Self {
            id: presentation_id,
            presenter_identity,
            clients: Arc::new(DashMap::new()),
            presenters: Arc::new(DashMap::new()),
            authentication_key,
            ratelimiter,
            slide_settings: Arc::new(None.into()),
            encrypted,
            presentation_data: PresentationData { title }
        }
    }
}
