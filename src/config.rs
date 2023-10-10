use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub service_address: String,
    pub new_presentation_authorization_key: String,
}

pub fn load_configuration<T>(path: T) -> Configuration
where
    T: AsRef<std::path::Path>,
{
    let configuration = std::fs::read_to_string(path).unwrap();
    toml::from_str(&configuration).unwrap()
}
