{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, ... }@inputs:

    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            inherit system;
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [
                inputs.self.overlays.default
              ];
            };
          }
        );
      serviceName = "webhashtag-rust-server";
    in
    {
      overlays.default = final: prev: {
        rustToolchain =
          with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable;
            [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
            ]
          );
        fenixRustPlatform = final.makeRustPlatform {
          cargo = final.rustToolchain;
          rustc = final.rustToolchain;
        };
      };

      packages = forEachSupportedSystem (
        { pkgs, ... }:
        {
          default = pkgs.fenixRustPlatform.buildRustPackage {
            pname = serviceName;
            version = "0.1.0";
            src = inputs.self;
            cargoLock.lockFile = ./Cargo.lock;
            meta.mainProgram = serviceName;
          };
        }
      );

      apps = forEachSupportedSystem (
        { system, ... }:
        {
          default = {
            type = "app";
            program = inputs.nixpkgs.lib.getExe self.packages.${system}.default;
            meta.description = "Run the WebHashtag tag server";
          };
        }
      );

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          ...
        }:
        let
          cfg = config.services.webhashtag-rust-server;
        in
        {
          options.services.webhashtag-rust-server = {
            enable = lib.mkEnableOption "WebHashtag tag server";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
              defaultText = lib.literalExpression "self.packages.\${pkgs.stdenv.hostPlatform.system}.default";
              description = "Package providing the webhashtag-rust-server binary.";
            };

            tags = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              example = [
                "rust"
                "typescript"
              ];
              description = "Tags accepted by this tag server.";
            };

            port = lib.mkOption {
              type = lib.types.port;
              default = 3000;
              description = "Port to listen on.";
            };

            openFirewall = lib.mkOption {
              type = lib.types.bool;
              default = false;
              description = "Open the configured port in the firewall.";
            };

            listenAddress = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
              example = "0.0.0.0";
              description = "Address the service binds to.";
            };

            serverHost = lib.mkOption {
              type = lib.types.str;
              default = "localhost:${toString cfg.port}";
              defaultText = lib.literalExpression ''"localhost:$\{toString config.services.webhashtag-rust-server.port}"'';
              description = "Public host used when validating tag declaration backlinks.";
            };

            serverName = lib.mkOption {
              type = lib.types.str;
              default = "Example Tag Server";
              description = "Human-readable server name exposed in webhashtag metadata.";
            };

            mode = lib.mkOption {
              type = lib.types.enum [
                "open"
                "closed"
              ];
              default = "open";
              description = "Registration mode.";
            };

            environmentFile = lib.mkOption {
              type = lib.types.nullOr lib.types.path;
              default = null;
              example = "/run/secrets/webhashtag-rust-server.env";
              description = "Optional systemd EnvironmentFile. Use this to provide SECRET_KEY in closed mode.";
            };
          };

          config = lib.mkIf cfg.enable {
            assertions = [
              {
                assertion = cfg.tags != [ ];
                message = "services.webhashtag-rust-server.tags must contain at least one tag.";
              }
              {
                assertion = cfg.mode != "closed" || cfg.environmentFile != null;
                message = "services.webhashtag-rust-server.environmentFile is required when mode is closed.";
              }
            ];

            systemd.services.webhashtag-rust-server = {
              description = "WebHashtag tag server";
              wantedBy = [ "multi-user.target" ];
              after = [ "network-online.target" ];
              wants = [ "network-online.target" ];

              environment = {
                TAGS = lib.concatStringsSep "," cfg.tags;
                PORT = toString cfg.port;
                BIND_ADDRESS = cfg.listenAddress;
                SERVER_HOST = cfg.serverHost;
                SERVER_NAME = cfg.serverName;
                MODE = cfg.mode;
              };

              serviceConfig = {
                ExecStart = lib.getExe cfg.package;
                DynamicUser = true;
                StateDirectory = serviceName;
                WorkingDirectory = "/var/lib/${serviceName}";
                Restart = "on-failure";
              }
              // lib.optionalAttrs (cfg.environmentFile != null) {
                EnvironmentFile = cfg.environmentFile;
              };
            };

            networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [ cfg.port ];
          };
        };

      devShells = forEachSupportedSystem (
        { pkgs, system }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              openssl
              pkg-config
              cargo-deny
              cargo-edit
              cargo-watch
              rust-analyzer
              self.formatter.${system}
            ];

            env = {
              # Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        }
      );

      formatter = forEachSupportedSystem ({ pkgs, ... }: pkgs.nixfmt);
    };
}
