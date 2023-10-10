#[macro_use]
extern crate log;

use dashmap::DashMap;
use exhibit::{
    config, handler, processor, Clients, ConfigurationMessage, IdentifiedUserMessage, JwtClaims,
    Presentation,
};
use jsonwebtoken::{decode, DecodingKey};
use std::convert::Infallible;
use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Configure Exhibit
    let config_path = std::env::args()
        .nth(1)
        .expect("Please provide a configuration file path");
    let configuration = config::load_configuration(&config_path);

    let mut presentations: DashMap<String, Presentation> = DashMap::new();
    // Create shared state
    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let presenters = Arc::new(RwLock::new(HashMap::new()));
    let (client_emoji_sender, client_emoji_receiver) =
        mpsc::unbounded_channel::<IdentifiedUserMessage>();
    let (configuration_sender, configuration_receiver) =
        mpsc::unbounded_channel::<ConfigurationMessage>();

    // APIs
    let health_route = warp::path!("health").and_then(handler::health_handler);
    let client_ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with(clients.clone()))
        .and_then(handler::client_ws_handler);
    let new_presentation = warp::path!("new")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 4))
        .and(with(presentations.clone()))
        .and_then(handler::new_presentation_hander);

    let register_route = warp::path!("register")
        // Set maximum request size
        .and(warp::body::content_length_limit(1024 * 2))
        .and(warp::post())
        .and(warp::body::bytes().and_then(move |provided_token| {
            parse_jwt_for_presentation(provided_token, presentations.clone())
        }))
        .and(with(clients.clone()))
        .and(with(client_emoji_sender.clone()))
        .and_then(handler::register_jwt_handler);

    // SPAs
    let client_spa = warp::path::end().and(warp::fs::file("web/client.html"));
    let presenter_spa = warp::path("present").and(warp::fs::file("web/present.html"));

    let client_routes = health_route
        .or(register_route)
        .or(client_ws_route)
        .or(client_spa);

    // Admin/Presenter routes
    let update = warp::path!("update")
        .and(warp::post())
        .and(warp::body::json())
        .and(with(clients.clone()))
        .and(with(configuration_sender.clone()))
        .and_then(handler::update_handler);

    let presenter_emoji_stream = warp::path("emoji_stream")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with(presenters.clone()))
        .and_then(handler::presenter_ws_handler);

    let presenter_routes = update.or(presenter_emoji_stream).or(presenter_spa);

    tokio::task::spawn(async move {
        processor::handle_sent_messages(client_emoji_receiver, configuration_receiver, presenters)
            .await;

        panic!("Emoji receiver was dropped?");
    });

    let all_routes = client_routes.or(presenter_routes);

    let service_address: SocketAddr = configuration.service_address.parse().unwrap();

    warp::serve(all_routes).run(service_address).await
}

async fn parse_jwt_for_presentation(
    provided_token: warp::hyper::body::Bytes,
    presentations: DashMap<String, Presentation>,
) -> Result<JwtClaims, warp::reject::Rejection> {
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);

    let token = String::from_utf8(provided_token.to_vec()).map_err(|e| {
        error!("User rejected due to non UTF8 JWT: {e}");
        warp::reject::not_found()
    })?;

    let header = jsonwebtoken::decode_header(&token).map_err(|_| warp::reject::not_found())?;
    let requested_presentation_id = header.kid.ok_or(warp::reject::not_found())?;

    let presentation = presentations
        .get(&requested_presentation_id)
        .ok_or(warp::reject::not_found())?;

    let token = decode::<JwtClaims>(
        &token,
        &DecodingKey::from_secret(presentation.value().client_authentication_key.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    Ok(token.claims)
}

fn with<T>(item: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Send + Sync + Clone,
{
    warp::any().map(move || item.clone())
}
