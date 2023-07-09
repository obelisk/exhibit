#[macro_use]
extern crate log;

use exhibit::{config, handler, processor, Clients, ConfigurationMessage, EmojiMessage, JwtClaims};
use jsonwebtoken::{decode, DecodingKey};
use std::convert::Infallible;
use std::sync::Arc;
use std::{collections::HashMap, net::SocketAddr};
use tokio::sync::{mpsc, RwLock};
use warp::Filter;

#[tokio::main]
async fn main() {
    env_logger::init();

    let config_path = std::env::args()
        .nth(1)
        .expect("Please provide a configuration file path");

    let configuration = config::load_configuration(&config_path);

    let clients: Clients = Arc::new(RwLock::new(HashMap::new()));
    let presenters = Arc::new(RwLock::new(HashMap::new()));

    let health_route = warp::path!("health").and_then(handler::health_handler);

    let (client_emoji_sender, client_emoji_receiver) = mpsc::unbounded_channel::<EmojiMessage>();
    let (configuration_sender, configuration_receiver) =
        mpsc::unbounded_channel::<ConfigurationMessage>();

    let client_ws_route = warp::path("ws")
        .and(warp::ws())
        .and(warp::path::param())
        .and(with(clients.clone()))
        .and_then(handler::client_ws_handler);

    let client_spa = warp::path::end().and(warp::fs::file("web/client.html"));

    let base_route = warp::path("register").and(warp::body::content_length_limit(1024 * 32));

    let possible_routes = match configuration.authentication_configuration {
        config::AuthenticationConfiguration::Header { header } => {
            let register_route = base_route
                .and(warp::get())
                .and(with(header))
                .and(warp::header::headers_cloned())
                .and(with(clients.clone()))
                .and(with(client_emoji_sender.clone()))
                .and_then(handler::register_header_handler);

            (
                Some(
                    health_route
                        .or(register_route)
                        .or(client_ws_route)
                        .or(client_spa),
                ),
                None,
            )
        }
        config::AuthenticationConfiguration::Jwt {
            public_key,
            audience,
        } => {
            let register_route = warp::path("register")
                // Set maximum request size
                .and(warp::body::content_length_limit(1024 * 32))
                .and(warp::post())
                .and(
                    warp::body::bytes()
                        .and_then(move |x| parse_jwt(x, public_key.clone(), audience.clone())),
                )
                .and(with(clients.clone()))
                .and(with(client_emoji_sender.clone()))
                .and_then(handler::register_jwt_handler);

            (
                None,
                Some(
                    health_route
                        .or(register_route)
                        .or(client_ws_route)
                        .or(client_spa),
                ),
            )
        }
    };

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

    let presenter_spa = warp::path("present").and(warp::fs::file("web/present.html"));

    let presenter_routes = update.or(presenter_emoji_stream).or(presenter_spa);

    tokio::task::spawn(async move {
        processor::handle_sent_emojis(client_emoji_receiver, configuration_receiver, presenters)
            .await;

        panic!("Emoji receiver was dropped?");
    });

    let client_address: SocketAddr = configuration.client_server_address.parse().unwrap();
    let presenter_address: SocketAddr = configuration.presentation_server_address.parse().unwrap();

    match possible_routes {
        (None, Some(cr)) => tokio::join!(
            warp::serve(cr).run(client_address),
            warp::serve(presenter_routes).run(presenter_address)
        ),
        (Some(cr), None) => tokio::join!(
            warp::serve(cr).run(client_address),
            warp::serve(presenter_routes).run(presenter_address)
        ),
        _ => unreachable!(
            "Configuration system is broken as authentication was not properly configured"
        ),
    };
}

async fn parse_jwt(
    token: warp::hyper::body::Bytes,
    private_key: String,
    audience: Option<String>,
) -> Result<JwtClaims, warp::reject::Rejection> {
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::ES256);

    if let Some(audience) = audience {
        validation.set_audience(&vec![audience]);
    };

    let token = String::from_utf8(token.to_vec()).map_err(|e| {
        error!("User rejected due to non UTF8 JWT: {e}");
        warp::reject::not_found()
    })?;
    let token = decode::<JwtClaims>(
        &token,
        &DecodingKey::from_ec_pem(private_key.as_bytes()).unwrap(),
        &validation,
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    Ok(token.claims)
}

fn with<T>(users: T) -> impl Filter<Extract = (T,), Error = Infallible> + Clone
where
    T: Send + Sync + Clone,
{
    warp::any().map(move || users.clone())
}
