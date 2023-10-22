use jsonwebtoken::DecodingKey;
use serde::{de::Error, Deserialize, Deserializer};

use base64::engine::general_purpose::STANDARD as base64decoder;
use base64::Engine;

#[derive(Clone, Deserialize)]
pub struct Configuration {
    pub service_address: String,
    pub service_port: u16,
    #[serde(deserialize_with = "deserialize_decoding_key")]
    pub new_presentation_signing_key: DecodingKey,
}

/// Fetch the Exhibit configuration. Check a path if one is provided, otherwise
/// look for a base64 encoded blob in the EXHIBIT_CONFIG environment variable.
/// 
/// Also allow overriding of the port because apparently that's how Heroku rolls.
pub fn load_configuration() -> Configuration{
    let configuration_toml = match std::env::args().nth(1) {
        Some(path) => std::fs::read_to_string(path).unwrap(),
        None => {
            let base64_config = std::env::var("EXHIBIT_CONFIG").unwrap();
            let config_bytes = base64decoder.decode(base64_config).unwrap();
            String::from_utf8(config_bytes).unwrap()
        },
    };

    let mut config: Configuration = toml::from_str(&configuration_toml).unwrap();

    if let Ok(port) = std::env::var("PORT") {
        config.service_port = port.parse().unwrap();
    }

    config
}

fn deserialize_decoding_key<'de, D>(deserializer: D) -> Result<DecodingKey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    DecodingKey::from_ec_pem(s.as_bytes()).map_err(D::Error::custom)
}
