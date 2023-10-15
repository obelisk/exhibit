use std::collections::HashMap;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};

use crate::{
    ClientJoinPresentationData, JwtClaims, NewPresentationRequest, Presentation, Presentations,
};

pub async fn join_presentation(
    provided_token: warp::hyper::body::Bytes,
    presentations: Presentations,
) -> Result<ClientJoinPresentationData, warp::reject::Rejection> {
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
        &DecodingKey::from_secret(presentation.value().authentication_key.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    Ok(ClientJoinPresentationData {
        presentation: requested_presentation_id,
        claims: token.claims,
    })
}

pub async fn new_presentation(
    new_presentation_signing_key: DecodingKey,
    new_presentation_request: HashMap<String, String>,
    presentations: Presentations,
) -> Result<Presentation, warp::reject::Rejection> {
    info!("New presentation requested!");
    let validation = Validation::new(Algorithm::ES256);
    println!("{:?}", new_presentation_request);
    // We could put this in a lazy static but since we instantiate new
    // presentations comparatively infrequently, keeping the JWT code
    // inside the authentication module I believe is preferable

    let new_presentation_request = NewPresentationRequest {
        token: new_presentation_request
            .get("registration_key")
            .ok_or(warp::reject())?
            .to_string(),
        presenter_identity: new_presentation_request
            .get("presenter_identity")
            .ok_or(warp::reject())?
            .to_string(),
        encrypted: new_presentation_request
            .get("encrypted")
            .ok_or(warp::reject())?
            == "on",
        authorization_key: new_presentation_request
            .get("authorization_public_key")
            .ok_or(warp::reject())?
            .to_string(),
    };

    let token = jsonwebtoken::decode::<JwtClaims>(
        &new_presentation_request.token,
        &new_presentation_signing_key,
        &validation,
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    // Check if that presentation already exists
    // If so, we breakout as we will not override that presentation
    if presentations.get(&token.claims.pid).is_some() {
        return Err(warp::reject::reject());
    }

    let presentation = Presentation::new(
        token.claims.pid,
        new_presentation_request.presenter_identity,
        new_presentation_request.encrypted,
        new_presentation_request.authorization_key,
    );

    debug!(
        "Creating new presentation {} with presenter {}. Authentication public key: {}",
        presentation.id, token.claims.sub, presentation.authentication_key,
    );

    Ok(presentation)
}
