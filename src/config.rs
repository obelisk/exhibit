use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Configuration {
    pub authentication_configuration: AuthenticationConfiguration,
    pub presentation_server_address: String,
    pub client_server_address: String,
}

#[derive(Clone, Debug, Deserialize)]
pub enum AuthenticationConfiguration {
    Header {
        header: String,
    },
    //Token {
    //    secret: String,
    //},
    Jwt {
        public_key: String,
        audience: Option<String>,
    },
}

pub fn load_configuration<T>(path: T) -> Configuration
where
    T: AsRef<std::path::Path>,
{
    let configuration = std::fs::read_to_string(path).unwrap();
    toml::from_str(&configuration).unwrap()
}