use dashmap::DashMap;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};

use crate::{JwtClaims, Presentation};

pub async fn parse_jwt_presentation_join(
    provided_token: warp::hyper::body::Bytes,
    presentations: DashMap<String, Presentation>,
) -> Result<JwtClaims, warp::reject::Rejection> {
    let validation = Validation::new(Algorithm::HS256);

    let token = String::from_utf8(provided_token.to_vec()).map_err(|e| {
        error!("User rejected due to non UTF8 JWT: {e}");
        warp::reject::not_found()
    })?;

    let header = jsonwebtoken::decode_header(&token).map_err(|_| warp::reject::not_found())?;
    let requested_presentation_id = header.kid.ok_or(warp::reject::not_found())?;

    let presentation = presentations
        .get(&requested_presentation_id)
        .ok_or(warp::reject::not_found())?;

    let token = jsonwebtoken::decode::<JwtClaims>(
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

pub async fn parse_jwt_presentation_new(
    authorization_key: String,
    provided_token: warp::hyper::body::Bytes,
    presentations: DashMap<String, Presentation>,
) -> Result<JwtClaims, warp::reject::Rejection> {
    let validation = Validation::new(Algorithm::HS256);

    let token = String::from_utf8(provided_token.to_vec()).map_err(|e| {
        error!("User rejected due to non UTF8 JWT: {e}");
        warp::reject::not_found()
    })?;

    let header = jsonwebtoken::decode_header(&token).map_err(|_| warp::reject::not_found())?;
    let requested_presentation_id = header.kid.ok_or(warp::reject::not_found())?;

    // Check if that presentation already exists
    // If so, we breakout as we will not override that presentation
    if presentations.get(&requested_presentation_id).is_some() {
        return Err(warp::reject::reject());
    }

    let token = jsonwebtoken::decode::<JwtClaims>(
        &token,
        &DecodingKey::from_secret(authorization_key.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    Ok(token.claims)
}
