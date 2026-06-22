use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;
use webhashtag_rust_server::{AppConfig, ArticleVerifier, OgpData, Store, TokenPayload, build_app};

#[derive(Clone)]
struct TestVerifier {
    backlinks: Arc<HashMap<String, bool>>,
    ogp: OgpData,
}

#[async_trait]
impl ArticleVerifier for TestVerifier {
    async fn verify_backlink(&self, article_url: &str, _tag_url: &str) -> bool {
        self.backlinks.get(article_url).copied().unwrap_or(false)
    }

    async fn fetch_ogp(&self, _article_url: &str) -> OgpData {
        self.ogp.clone()
    }
}

fn verifier_for(article_url: &str, has_backlink: bool) -> TestVerifier {
    TestVerifier {
        backlinks: Arc::new(HashMap::from([(article_url.to_string(), has_backlink)])),
        ogp: OgpData {
            title: Some("Rust Post".to_string()),
            description: Some("A post about Rust".to_string()),
            image: Some("https://example.com/og.png".to_string()),
        },
    }
}

async fn test_app(config: AppConfig, verifier: TestVerifier) -> (axum::Router, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::load(dir.path()).expect("store loads");
    let app = build_app(config, store, Arc::new(verifier));
    (app, dir)
}

async fn get_json(app: axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let value = serde_json::from_slice(&body).expect("json body");
    (status, value)
}

async fn get_text(app: axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, String::from_utf8(body.to_vec()).expect("utf8 body"))
}

#[tokio::test]
async fn exposes_server_metadata() {
    let article_url = "https://example.com/post";
    let config = AppConfig::open(
        "tag.example.com".to_string(),
        "Programming Tags".to_string(),
        vec!["rust".to_string(), "go".to_string()],
    );
    let (app, _dir) = test_app(config, verifier_for(article_url, true)).await;

    let (status, body) = get_json(app, "/.well-known/webhashtag.json").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "version": "0.1",
            "name": "Programming Tags",
            "mode": "open",
            "publicKey": null,
            "tags": ["rust", "go"]
        })
    );
}

#[tokio::test]
async fn registers_article_when_backlink_exists_and_publishes_json_and_atom() {
    let article_url = "https://example.com/post";
    let config = AppConfig::open(
        "tag.example.com".to_string(),
        "Programming Tags".to_string(),
        vec!["rust".to_string()],
    );
    let (app, dir) = test_app(config, verifier_for(article_url, true)).await;
    let uri = format!("/declare/rust?url={}", urlencoding::encode(article_url));

    let (status, body) = get_json(app.clone(), &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": "registered"}));

    let (status, body) = get_json(app.clone(), "/tag/rust").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["tag"], "rust");
    assert_eq!(body["server"], "tag.example.com");
    assert_eq!(body["entries"][0]["url"], article_url);
    assert_eq!(body["entries"][0]["ogp"]["title"], "Rust Post");

    let (status, atom) = get_text(app, "/feed/rust").await;
    assert_eq!(status, StatusCode::OK);
    assert!(atom.contains("<feed"));
    assert!(atom.contains("https://example.com/post"));
    assert!(dir.path().join("data/webhashtag.redb").exists());
    assert!(!dir.path().join("public/tag/rust.json").exists());
    assert!(!dir.path().join("public/feed/rust.atom").exists());
}

#[tokio::test]
async fn persists_entries_after_reopening_the_store() {
    let article_url = "https://example.com/post";
    let config = AppConfig::open(
        "tag.example.com".to_string(),
        "Programming Tags".to_string(),
        vec!["rust".to_string()],
    );
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::load(dir.path()).expect("store loads");
    let app = build_app(
        config.clone(),
        store,
        Arc::new(verifier_for(article_url, true)),
    );
    let uri = format!("/declare/rust?url={}", urlencoding::encode(article_url));
    let (status, _) = get_json(app, &uri).await;
    assert_eq!(status, StatusCode::OK);

    let reopened = Store::load(dir.path()).expect("store reopens");
    let reopened_app = build_app(config, reopened, Arc::new(verifier_for(article_url, true)));
    let (status, body) = get_json(reopened_app, "/tag/rust").await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["entries"][0]["url"], article_url);
}

#[tokio::test]
async fn rejects_unknown_tags_and_missing_backlinks() {
    let article_url = "https://example.com/post";
    let config = AppConfig::open(
        "tag.example.com".to_string(),
        "Programming Tags".to_string(),
        vec!["rust".to_string()],
    );
    let (app, _dir) = test_app(config, verifier_for(article_url, false)).await;

    let unknown_uri = format!("/declare/go?url={}", urlencoding::encode(article_url));
    let (status, body) = get_json(app.clone(), &unknown_uri).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({"error": "Tag not found"}));

    let rust_uri = format!("/declare/rust?url={}", urlencoding::encode(article_url));
    let (status, body) = get_json(app, &rust_uri).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, json!({"error": "Tag link not found in the article"}));
}

#[tokio::test]
async fn rejects_localhost_and_ip_article_urls() {
    let config = AppConfig::open(
        "tag.example.com".to_string(),
        "Programming Tags".to_string(),
        vec!["rust".to_string()],
    );
    let (app, _dir) = test_app(config, verifier_for("https://example.com/post", true)).await;

    let (status, body) = get_json(app.clone(), "/declare/rust?url=http://localhost/post").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error": "Invalid url parameter"}));

    let (status, body) = get_json(app, "/declare/rust?url=http://127.0.0.1/post").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error": "Invalid url parameter"}));
}

#[tokio::test]
async fn closed_mode_requires_a_valid_signed_token_bound_to_the_article() {
    let article_url = "https://example.com/post";
    let secret_key = [7_u8; 32];
    let config = AppConfig::closed(
        "tag.example.com".to_string(),
        "Private Tags".to_string(),
        vec!["rust".to_string()],
        hex::encode(secret_key),
    )
    .expect("closed config");
    let (app, _dir) = test_app(config, verifier_for(article_url, true)).await;
    let token = signed_token(&secret_key, article_url, "rust", "tag.example.com");

    let missing_uri = format!("/declare/rust?url={}", urlencoding::encode(article_url));
    let (status, body) = get_json(app.clone(), &missing_uri).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({"error": "Token is required"}));

    let uri = format!(
        "/declare/rust?url={}&token={}",
        urlencoding::encode(article_url),
        urlencoding::encode(&token)
    );
    let (status, body) = get_json(app, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"status": "registered"}));
}

fn signed_token(secret_key: &[u8; 32], url: &str, tag: &str, server: &str) -> String {
    let exp = (Utc::now() + Duration::days(1)).to_rfc3339();
    let payload = TokenPayload {
        url: url.to_string(),
        tag: tag.to_string(),
        server: server.to_string(),
        exp,
    };
    let message = serde_json::to_vec(&payload).expect("payload json");
    let signing_key = SigningKey::from_bytes(secret_key);
    let sig = signing_key.sign(&message);
    let token = json!({
        "url": payload.url,
        "tag": payload.tag,
        "server": payload.server,
        "exp": payload.exp,
        "sig": multibase::encode(multibase::Base::Base58Btc, sig.to_bytes()),
    });
    multibase::encode(
        multibase::Base::Base58Btc,
        serde_json::to_vec(&token).expect("token json"),
    )
}
