mod atom;
mod config;
mod revalidate;
mod store;
mod token;
mod verify;

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

pub use config::{AppConfig, Mode};
pub use revalidate::spawn_revalidation;
pub use store::Store;
pub use token::TokenPayload;
pub use verify::{ArticleVerifier, HttpArticleVerifier, OgpData};

#[derive(Clone)]
struct AppState {
    config: AppConfig,
    store: Store,
    verifier: Arc<dyn ArticleVerifier>,
}

#[derive(Deserialize)]
struct DeclareQuery {
    url: String,
    token: Option<String>,
}

pub fn build_app(config: AppConfig, store: Store, verifier: Arc<dyn ArticleVerifier>) -> Router {
    store.set_server_host(config.server_host());
    let state = AppState {
        config,
        store,
        verifier,
    };

    Router::new()
        .route("/.well-known/webhashtag.json", get(metadata))
        .route("/declare/{tag}", get(declare_tag))
        .route("/tag/{tag}", get(tag_json))
        .route("/feed/{tag}", get(feed_atom))
        .with_state(state)
}

async fn metadata(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.config.metadata())
}

async fn tag_json(State(state): State<AppState>, Path(tag): Path<String>) -> Response {
    if !is_valid_tag(&tag) {
        return json_error(StatusCode::BAD_REQUEST, "Invalid tag");
    }

    let Some(entries) = state.store.entries(&tag) else {
        return json_error(StatusCode::NOT_FOUND, "Tag not found");
    };

    (
        StatusCode::OK,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "tag": tag,
            "server": state.config.server_host(),
            "entries": entries,
        })),
    )
        .into_response()
}

async fn feed_atom(State(state): State<AppState>, Path(tag): Path<String>) -> Response {
    if !is_valid_tag(&tag) {
        return json_error(StatusCode::BAD_REQUEST, "Invalid tag");
    }

    let Some(atom) = state.store.atom(&tag) else {
        return json_error(StatusCode::NOT_FOUND, "Tag not found");
    };

    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::CONTENT_TYPE, "application/atom+xml; charset=utf-8"),
        ],
        Body::from(atom),
    )
        .into_response()
}

async fn declare_tag(
    State(state): State<AppState>,
    Path(tag): Path<String>,
    Query(query): Query<DeclareQuery>,
) -> Response {
    if !is_valid_tag(&tag) {
        return json_error(StatusCode::BAD_REQUEST, "Invalid tag");
    }

    if !state.config.accepts_tag(&tag) {
        return json_error(StatusCode::NOT_FOUND, "Tag not found");
    }

    let Ok(article_url) = normalize_article_url(&query.url) else {
        return json_error(StatusCode::BAD_REQUEST, "Invalid url parameter");
    };

    if state.config.is_closed() {
        let Some(token) = query.token.as_deref() else {
            return json_error(StatusCode::UNAUTHORIZED, "Token is required");
        };

        if let Err(error) = token::verify_token(&state.config, token, &article_url, &tag) {
            return json_error(error.status, error.message);
        }
    }

    if state.store.has_entry(&tag, &article_url) {
        return (
            StatusCode::OK,
            Json(json!({"status": "already registered"})),
        )
            .into_response();
    }

    let tag_url = format!("https://{}/declare/{}", state.config.server_host(), tag);
    if !state.verifier.verify_backlink(&article_url, &tag_url).await {
        return json_error(StatusCode::FORBIDDEN, "Tag link not found in the article");
    }

    let ogp = state.verifier.fetch_ogp(&article_url).await;
    match state.store.add_entry(&tag, &article_url, ogp) {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "registered"}))).into_response(),
        Err(error) => json_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

fn json_error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

pub fn is_valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn normalize_article_url(raw: &str) -> Result<String, url::ParseError> {
    let url = url::Url::parse(raw)?;
    let Some(host) = url.host_str() else {
        return Err(url::ParseError::EmptyHost);
    };

    if !matches!(url.scheme(), "http" | "https")
        || host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::IpAddr>().is_ok()
    {
        return Err(url::ParseError::RelativeUrlWithoutBase);
    }

    Ok(url.to_string())
}
