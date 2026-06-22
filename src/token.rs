use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, SigningKey, Verifier};
use serde::{Deserialize, Serialize};

use crate::{AppConfig, is_valid_tag, normalize_article_url};

#[derive(Debug)]
pub struct TokenError {
    pub status: StatusCode,
    pub message: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TokenPayload {
    pub url: String,
    pub tag: String,
    pub server: String,
    pub exp: String,
}

#[derive(Deserialize)]
struct SignedToken {
    url: String,
    tag: String,
    server: String,
    exp: String,
    sig: String,
}

pub fn verify_token(
    config: &AppConfig,
    encoded: &str,
    article_url: &str,
    requested_tag: &str,
) -> Result<(), TokenError> {
    let decoded = decode_base58btc(encoded).map_err(|_| invalid_format())?;
    let token: SignedToken = serde_json::from_slice(&decoded).map_err(|_| invalid_format())?;

    if normalize_article_url(&token.url).map_err(|_| invalid_format())? != article_url {
        return Err(unauthorized("Token URL does not match url parameter"));
    }
    if !is_valid_tag(&token.tag) {
        return Err(invalid_format());
    }
    if token.tag != requested_tag {
        return Err(unauthorized("Token tag does not match requested tag"));
    }
    if token.server != config.server_host() {
        return Err(unauthorized("Token server does not match this server"));
    }

    let exp = DateTime::parse_from_rfc3339(&token.exp)
        .map_err(|_| invalid_format())?
        .with_timezone(&Utc);
    if exp < Utc::now() {
        return Err(unauthorized("Token expired"));
    }

    let payload = TokenPayload {
        url: token.url,
        tag: token.tag,
        server: token.server,
        exp: token.exp,
    };
    let message = serde_json::to_vec(&payload).map_err(|_| invalid_format())?;
    let signature_bytes = decode_base58btc(&token.sig).map_err(|_| invalid_format())?;
    let signature_bytes: [u8; 64] = signature_bytes.try_into().map_err(|_| invalid_format())?;
    let signature = Signature::from_bytes(&signature_bytes);

    let Some(secret_key) = config.secret_key() else {
        return Err(TokenError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Server misconfigured",
        });
    };
    let verifying_key = SigningKey::from_bytes(&secret_key).verifying_key();
    verifying_key
        .verify(&message, &signature)
        .map_err(|_| unauthorized("Invalid token signature"))
}

fn decode_base58btc(encoded: &str) -> Result<Vec<u8>, multibase::Error> {
    let (_base, bytes) = multibase::decode(encoded)?;
    Ok(bytes)
}

fn invalid_format() -> TokenError {
    TokenError {
        status: StatusCode::BAD_REQUEST,
        message: "Invalid token format",
    }
}

fn unauthorized(message: &'static str) -> TokenError {
    TokenError {
        status: StatusCode::UNAUTHORIZED,
        message,
    }
}
