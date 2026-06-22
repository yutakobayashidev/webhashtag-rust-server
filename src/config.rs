use ed25519_dalek::SigningKey;
use serde::Serialize;

use crate::is_valid_tag;

#[derive(Clone)]
pub struct AppConfig {
    server_host: String,
    server_name: String,
    mode: Mode,
    tags: Vec<String>,
    secret_key: Option<[u8; 32]>,
    public_key: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Open,
    Closed,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerMetadata {
    version: &'static str,
    name: String,
    mode: Mode,
    public_key: Option<String>,
    tags: Vec<String>,
}

impl AppConfig {
    pub fn open(server_host: String, server_name: String, tags: Vec<String>) -> Self {
        Self {
            server_host,
            server_name,
            mode: Mode::Open,
            tags: sanitize_tags(tags),
            secret_key: None,
            public_key: None,
        }
    }

    pub fn closed(
        server_host: String,
        server_name: String,
        tags: Vec<String>,
        secret_key_hex: String,
    ) -> Result<Self, String> {
        let bytes = hex::decode(secret_key_hex).map_err(|error| error.to_string())?;
        let secret_key: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "SECRET_KEY must decode to 32 bytes".to_string())?;
        let public_key = SigningKey::from_bytes(&secret_key)
            .verifying_key()
            .to_bytes();

        Ok(Self {
            server_host,
            server_name,
            mode: Mode::Closed,
            tags: sanitize_tags(tags),
            secret_key: Some(secret_key),
            public_key: Some(multibase::encode(multibase::Base::Base58Btc, public_key)),
        })
    }

    pub fn metadata(&self) -> ServerMetadata {
        ServerMetadata {
            version: "0.1",
            name: self.server_name.clone(),
            mode: self.mode,
            public_key: self.public_key.clone(),
            tags: self.tags.clone(),
        }
    }

    pub fn accepts_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|allowed| allowed == tag)
    }

    pub fn is_closed(&self) -> bool {
        self.mode == Mode::Closed
    }

    pub fn server_host(&self) -> &str {
        &self.server_host
    }

    pub fn secret_key(&self) -> Option<[u8; 32]> {
        self.secret_key
    }
}

fn sanitize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter().filter(|tag| is_valid_tag(tag)).collect()
}
