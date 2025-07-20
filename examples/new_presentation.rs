use std::{fmt::Display, io::Write};

use jsonwebtoken::EncodingKey;

fn get_stdin(message: impl Display, default: Option<impl Display>) -> String {
    print!("{message}");
    if let Some(ref default) = &default {
        print!(" [default: {default}]");
    }
    print!(": ");

    std::io::stdout().flush().expect("Failed to flush stdout");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).expect("Failed to read line");
    let trimmed = input.trim().to_string();
    
    // If a default is provided and the input was empty, use the default
    if trimmed.is_empty() && default.is_some() {
        default.unwrap().to_string()
    } else {
        trimmed
    }
}

// Get Multiline from stdin. Returns on double newline.
fn get_multiline_stdin(message: impl Display) -> String {
    println!("{message}");
    let mut input = String::new();
    loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).expect("Failed to read line");
        if line.trim().is_empty() {
            break;
        }
        input.push_str(&line);
    }
    input
}

fn main() {
    println!("Generate new presentation keys");
    let exhibit_signing_key = get_multiline_stdin("Enter the configured Exhibit presentation signing key in PEM format (private key):");

    // This is the private portion of the key configured in the Exhibit configuration.
    // It's used to generate keys that allow for people to create new presentations.
    // We need it to generate the registration token for www.xzbt.live/new
    let exhibit_signing_key = match EncodingKey::from_ec_pem(exhibit_signing_key.as_bytes()){
        Ok(key) => key,
        Err(err) => {
            eprintln!("Failed to read signing key: {err}");
            return;
        }
    };

    // This is the private portion of the key used to sign people into the presentation.
    // You will submit the public portion of this to Exhibit when you create a new presentation
    // at www.xzbt.live/new
    let presentation_auth_key = get_multiline_stdin("Enter the presentation authentication key in PEM format (private key):");
    let presentation_auth_key = match EncodingKey::from_ec_pem(presentation_auth_key.as_bytes()){
        Ok(key) => key,
        Err(err) => {
            eprintln!("Failed to read authentication key: {err}");
            return;
        }
    };

    // This is the presentation ID that will uniquely identify your presentation. It needs to be unique
    // otherwise you will conflict with existing presentations (especially simoultaneous ones) and result in
    // unexpected behavior.
    let presentation_id = get_stdin("Enter the presentation ID", Some("5d313aea523d41569469e4abd72028d2"));

    // This is the user ID of the presentation owner. Currently it's not really used but does appear in logs
    let sub = get_stdin("Enter the user ID", Some("test"));

    // This is the identify of the presenter. Since viewers and presenters communicate with the backend using
    // the same endpoints, we need to tell the backend who to accept presenter messages from (such as new slides, polls, etx).
    // Anyone connecting to the presentation with this identity will have access to these APIs (and not standard client ones).
    let presenter_identity = get_stdin("Enter the presenter identity", Some("presenter"));

    // This is the additional identities that will be used to authenticate viewers of the presentation
    // (generally for testing).
    let additional_identities = get_stdin("Enter additional identities (comma separated)", Some("alpha,beta,gamma,delta"));

    // We generate the registration token that will be used to register the presentation
    let mut reg_header = jsonwebtoken::Header::default();
    reg_header.alg = jsonwebtoken::Algorithm::ES256;
    let registration_token = jsonwebtoken::encode(
        &reg_header,
        &exhibit::JwtClaims {
            sub,
            pid: presentation_id.clone(),
            exp: 2_000_000_000, // Far in the future
        },
        &exhibit_signing_key,
    ).expect("Failed to generate registration token");

    let mut header = jsonwebtoken::Header::default();
    header.alg = jsonwebtoken::Algorithm::ES256;
    header.kid = Some(presentation_id.clone());
    // Generate the presenter key that will be used to authenticate the presenter
    let presenter_key = jsonwebtoken::encode(
        &header,
        &exhibit::JwtClaims {
            sub: presenter_identity.clone(),
            pid: presentation_id.clone(),
            exp: 2_000_000_000, // Far in the future
        },
        &presentation_auth_key,
    ).expect("Failed to generate presenter key");

    println!("Presentation Registration Token (for www.xzbt.live/new):\n{registration_token}\n");
    println!("Presenter Key (for www.xzbt.live/present):\n{presenter_key}\n");

    for identity in additional_identities.split(',') {
        let additional_key = jsonwebtoken::encode(
            &header,
            &exhibit::JwtClaims {
                sub: identity.trim().to_string(),
                pid: presentation_id.clone(),
                exp: 2_000_000_000, // Far in the future
            },
            &presentation_auth_key,
        ).expect("Failed to generate additional identity key");
        println!("Additional Identity Key for {identity}:\n{additional_key}\n");
    }
}