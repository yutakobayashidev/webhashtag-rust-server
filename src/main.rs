use std::env;
use std::error::Error;
use std::io::{Error as IoError, ErrorKind};
use std::sync::Arc;

use tokio::net::TcpListener;
use webhashtag_rust_server::{
    AppConfig, HttpArticleVerifier, Store, build_app, spawn_revalidation,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "localhost:3000".to_string());
    let server_name = env::var("SERVER_NAME").unwrap_or_else(|_| "Example Tag Server".to_string());
    let tags = tags_from_env()?;
    let config = match env::var("MODE")
        .unwrap_or_else(|_| "open".to_string())
        .as_str()
    {
        "open" => AppConfig::open(server_host, server_name, tags),
        "closed" => AppConfig::closed(
            server_host,
            server_name,
            tags,
            env::var("SECRET_KEY").map_err(|_| input_error("SECRET_KEY env var is required"))?,
        )
        .map_err(input_error)?,
        other => return Err(input_error(format!("unsupported MODE: {other}"))),
    };

    let store = Store::load(".")?;
    let verifier = Arc::new(HttpArticleVerifier::new());
    let app = build_app(config.clone(), store.clone(), verifier.clone());
    let _revalidation = spawn_revalidation(config.clone(), store, verifier);

    let port = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .map_err(|_| input_error("PORT must be a number from 0 to 65535"))?;
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!(
        "Tag server running on http://localhost:{port} (mode: {})",
        if config.is_closed() { "closed" } else { "open" }
    );
    axum::serve(listener, app).await?;

    Ok(())
}

fn tags_from_env() -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
    let tags = env::var("TAGS").map_err(|_| input_error("TAGS env var is required"))?;
    let tags = tags
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if tags.is_empty() {
        return Err(input_error("TAGS must include at least one tag"));
    }
    Ok(tags)
}

fn input_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    IoError::new(ErrorKind::InvalidInput, message.into()).into()
}
