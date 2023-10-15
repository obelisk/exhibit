use jsonwebtoken::DecodingKey;
use serde::{de::Error, Deserialize, Deserializer};

#[derive(Clone, Deserialize)]
pub struct Configuration {
    pub service_address: String,
    #[serde(deserialize_with = "deserialize_decoding_key")]
    pub new_presentation_signing_key: DecodingKey,
}

pub fn load_configuration<T>(path: T) -> Configuration
where
    T: AsRef<std::path::Path>,
{
    let configuration = std::fs::read_to_string(path).unwrap();
    toml::from_str(&configuration).unwrap()
}

fn deserialize_decoding_key<'de, D>(deserializer: D) -> Result<DecodingKey, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    DecodingKey::from_ec_pem(s.as_bytes()).map_err(D::Error::custom)
}
