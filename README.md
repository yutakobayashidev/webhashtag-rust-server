# WebHashtag Rust Server

Rust implementation of the WebHashtag tag server draft in [`marukun712/WebHashtag`](https://github.com/marukun712/WebHashtag/blob/main/spec/DRAFT.md).

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
By default it binds to `127.0.0.1`. Set `BIND_ADDRESS=0.0.0.0` only when the service should be reachable from other machines.

You can also run the flake package directly:

```sh
TAGS=rust,typescript nix run
```

## NixOS Service

Import `nixosModules.default` and enable the service:

```nix
{
  inputs.webhashtag-rust-server.url = "github:yutakobayashidev/webhashtag-rust-server";

  outputs = { self, nixpkgs, webhashtag-rust-server, ... }: {
    nixosConfigurations.example = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        webhashtag-rust-server.nixosModules.default
        {
          services.webhashtag-rust-server = {
            enable = true;
            tags = [ "rust" "typescript" ];
            serverHost = "tag.example.com";
            serverName = "Example Tag Server";
            port = 3000;
            listenAddress = "0.0.0.0";
            openFirewall = true;
          };
        }
      ];
    };
  };
}
```

The service runs with `DynamicUser` and stores redb data under `/var/lib/webhashtag-rust-server/data/webhashtag.redb`.
With the default `openFirewall = false`, it binds to `127.0.0.1` and does not change firewall rules.
Set `listenAddress = "0.0.0.0"` to bind to all interfaces.
Set `openFirewall = true` to add the configured port to `networking.firewall.allowedTCPPorts`.

For closed mode, provide `SECRET_KEY` through an environment file:

```nix
services.webhashtag-rust-server = {
  enable = true;
  tags = [ "rust" ];
  mode = "closed";
  environmentFile = "/run/secrets/webhashtag-rust-server.env";
};
```

## Environment

- `TAGS` is required. Use a comma-separated list such as `rust,typescript,go`.
- `SERVER_HOST` defaults to `localhost:3000`.
- `SERVER_NAME` defaults to `Example Tag Server`.
- `PORT` defaults to `3000`.
- `BIND_ADDRESS` defaults to `127.0.0.1`.
- `MODE` defaults to `open`. Set `MODE=closed` to require signed tokens.
- `SECRET_KEY` is required in closed mode. It must be a 32-byte Ed25519 secret key encoded as hex.

## Development

```sh
cargo test
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
```
