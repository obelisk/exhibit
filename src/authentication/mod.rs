use std::collections::HashMap;

use jsonwebtoken::{Algorithm, DecodingKey, Validation};

use crate::{ClientJoinPresentationData, JwtClaims, Presentation, Presentations};

pub async fn join_presentation(
    token: warp::hyper::body::Bytes,
    presentations: Presentations,
) -> Result<ClientJoinPresentationData, warp::reject::Rejection> {
    // Pull the token out of the request, this will have had to be
    // signed by the owner of the service so we can fail fast if it's
    // not valid
    let token = String::from_utf8(token.to_vec()).map_err(|e| {
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
        &presentation.authentication_key,
        &Validation::new(Algorithm::ES256),
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
    request: HashMap<String, String>,
    presentations: Presentations,
) -> Result<Presentation, warp::reject::Rejection> {
    // Pull the token out of the request, this will have had to be
    // signed by the owner of the service so we can fail fast if it's
    // not valid
    let token = request
        .get("registration_key")
        .ok_or(warp::reject())?
        .to_string();

    // Validate the token and pull out the claims if validation succeeds
    let token = jsonwebtoken::decode::<JwtClaims>(
        &token,
        &new_presentation_signing_key,
        &Validation::new(Algorithm::ES256),
    )
    .map_err(|e| {
        error!("User rejected due to JWT error: {e}");
        warp::reject::not_found()
    })?;

    let user_authorization_key = request
        .get("authorization_public_key")
        .ok_or(warp::reject())?
        .to_string();

    let presenter_identity = request
        .get("presenter_identity")
        .ok_or(warp::reject())?
        .to_string();

    let encrypted = request
        .get("encrypted")
        .map(|x| x.as_str())
        .unwrap_or("off")
        == "on";

    let title = request
        .get("title")
        .ok_or(warp::reject())?
        .to_string();

    let authentication_key =
        DecodingKey::from_ec_pem(user_authorization_key.as_bytes()).map_err(|_| warp::reject())?;

    // Check if that presentation already exists
    // If so, we breakout as we will not override that presentation
    if presentations.get(&token.claims.pid).is_some() {
        return Err(warp::reject::reject());
    }

    // That if statement is super ugly but it currently feels better than duplicating the debug line
    debug!(
        "[{}] creating new {}presentation [{}] with presenter [{presenter_identity}]. Authentication public key: {user_authorization_key}",
        token.claims.sub,
        if encrypted {
            "encrypted "
        } else {
            ""
        },
        token.claims.pid,
    );

    Ok(Presentation::new(
        token.claims.pid,
        presenter_identity,
        encrypted,
        authentication_key,
        title,
    ))
}
