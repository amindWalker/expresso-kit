{
  description = "EspressoKit - Automated project validator for local/cloud environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    crane = {
      url = "github:ipetkov/crane";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Use stable Rust with specific version for reproducibility
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
        };

        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Filter source to only include Rust-relevant files
        src = pkgs.lib.cleanSourceWith {
          src = craneLib.path ./.;
          filter = path: type:
            (craneLib.filterCargoSources path type)
            || (builtins.match ".*\\.md$" path != null)
            || (builtins.match ".*\\.toml$" path != null);
        };

        commonArgs = {
          inherit src;
          pname = "expresso-kit";
          strictDeps = true;

          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.apple-sdk
            pkgs.libiconv
          ];

          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        expresso-kit = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          doCheck = true;

          postInstall = ''
            # Install documentation
            install -Dm644 README.md -t $out/share/doc/expresso-kit/
            install -Dm644 GUIDE.md -t $out/share/doc/expresso-kit/ || true
          '';

          meta = with pkgs.lib; {
            description = "Automated project validator for local/cloud environments with TUI and CLI";
            longDescription = ''
              EspressoKit validates repository configurations for local/cloud environments,
              testing against CI deployment to guarantee idempotent and reproducible results.
            '';
            homepage = "https://github.com/amindWalker/expresso-kit";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "expresso-kit";
            platforms = platforms.unix;
          };
        });

        # Checks
        checks = {
          inherit expresso-kit;

          clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          fmt = craneLib.cargoFmt { inherit src; };

          doc = craneLib.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });
        };

      in {
        packages = {
          default = expresso-kit;
          inherit expresso-kit;
        };

        inherit checks;

        apps.default = flake-utils.lib.mkApp {
          drv = expresso-kit;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs; [
            rustToolchain
            rust-analyzer
            cargo-watch
            cargo-edit
            cargo-audit
            cargo-nextest
            git
            docker
            docker-compose
          ];

          shellHook = ''
            echo "🦀 Expresso-Kit Development Environment"
            echo "Rust: $(rustc --version)"
            echo ""
          '';
        };
      }
    ) // {
      overlays.default = final: prev: {
        expresso-kit = self.packages.${final.system}.expresso-kit;
      };
    };
}
