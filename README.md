# WebHashtag Rust Server

Rust implementation of the WebHashtag tag server draft in `WebHashtag/spec/DRAFT.md`.

## Features

- `GET /.well-known/webhashtag.json`
- `GET /declare/{tag}?url={articleURL}`
- `GET /tag/{tag}`
- `GET /feed/{tag}`
- Backlink verification before registration
- redb persistence under `data/webhashtag.redb`
- Open mode and closed mode Ed25519 token verification
- Weekly background revalidation of registered backlinks

## Run

```sh
TAGS=rust,typescript cargo run
```

The server listens on port `3000` by default.

## Environment

- `TAGS` is required. Use a comma-separated list such as `rust,typescript,go`.
- `SERVER_HOST` defaults to `localhost:3000`.
- `SERVER_NAME` defaults to `Example Tag Server`.
- `PORT` defaults to `3000`.
- `MODE` defaults to `open`. Set `MODE=closed` to require signed tokens.
- `SECRET_KEY` is required in closed mode. It must be a 32-byte Ed25519 secret key encoded as hex.

## Development

```sh
cargo test
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
```
