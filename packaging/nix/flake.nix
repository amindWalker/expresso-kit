{
  description = "EspressoKit - Automated project validator for local/cloud environments";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    # For better Rust builds
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Use the Rust version specified in rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ../../rust-toolchain.toml;

        # Crane for better Rust builds
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Common arguments for crane builds
        commonArgs = {
          src = craneLib.cleanCargoSource (craneLib.path ../..);

          strictDeps = true;

          buildInputs = with pkgs; [
            # Add runtime dependencies here
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.libiconv
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
        };

        # Build just the cargo dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        # Build the actual package
        expresso-kit = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          # Add metadata
          meta = with pkgs.lib; {
            description = "Automated project validator for local/cloud environments";
            homepage = "https://github.com/amindWalker/expresso-kit";
            license = licenses.mit;
            maintainers = [ ];
            mainProgram = "expresso-kit";
          };
        });

        # Clippy checks
        expresso-kit-clippy = craneLib.cargoClippy (commonArgs // {
          inherit cargoArtifacts;
          cargoClippyExtraArgs = "--all-targets -- --deny warnings";
        });

        # Format check
        expresso-kit-fmt = craneLib.cargoFmt {
          src = craneLib.cleanCargoSource (craneLib.path ../..);
        };

        # Tests
        expresso-kit-tests = craneLib.cargoNextest (commonArgs // {
          inherit cargoArtifacts;
          partitions = 1;
          partitionType = "count";
        });

      in
      {
        # Packages
        packages = {
          default = expresso-kit;
          expresso-kit = expresso-kit;
        };

        # Checks for CI
        checks = {
          inherit expresso-kit expresso-kit-clippy expresso-kit-fmt;
          # Uncomment when tests are stable:
          # inherit expresso-kit-tests;
        };

        # Apps
        apps.default = flake-utils.lib.mkApp {
          drv = expresso-kit;
        };

        # Development shell
        devShells.default = craneLib.devShell {
          # Inherit checks for pre-commit
          checks = self.checks.${system};

          # Extra packages for development
          packages = with pkgs; [
            # Rust tools
            rustToolchain
            rust-analyzer
            cargo-watch
            cargo-edit
            cargo-audit
            cargo-outdated
            cargo-nextest

            # Development tools
            git
            docker
            docker-compose

            # Formatting and linting
            nixpkgs-fmt

            # Shell completion generators
            installShellFiles
          ];

          # Environment variables
          shellHook = ''
            echo "🦀 Expresso-Kit Development Environment"
            echo "Rust: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo ""
            echo "Available commands:"
            echo "  cargo build    - Build the project"
            echo "  cargo test     - Run tests"
            echo "  cargo clippy   - Run linter"
            echo "  cargo run      - Run the application"
            echo "  nix build      - Build with Nix"
            echo "  nix flake check - Run all checks"
          '';
        };
      }
    ) // {
      # Overlay for use in other flakes
      overlays.default = final: prev: {
        expresso-kit = self.packages.${final.system}.expresso-kit;
      };

      # NixOS/Home Manager module
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.expresso-kit;
        in
        {
          options.programs.expresso-kit = {
            enable = lib.mkEnableOption "expresso-kit project validator";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.expresso-kit;
              description = "The expresso-kit package to use.";
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ cfg.package ];
          };
        };

      # Home Manager module
      homeManagerModules.default = { config, lib, pkgs, ... }:
        let
          cfg = config.programs.expresso-kit;
        in
        {
          options.programs.expresso-kit = {
            enable = lib.mkEnableOption "expresso-kit project validator";

            package = lib.mkOption {
              type = lib.types.package;
              default = self.packages.${pkgs.system}.expresso-kit;
              description = "The expresso-kit package to use.";
            };

            settings = lib.mkOption {
              type = lib.types.attrs;
              default = { };
              description = "Configuration for expresso-kit.";
            };
          };

          config = lib.mkIf cfg.enable {
            home.packages = [ cfg.package ];

            # Write config file if settings are provided
            xdg.configFile."expresso-kit/config.toml" = lib.mkIf (cfg.settings != { }) {
              text = lib.generators.toTOML { } cfg.settings;
            };
          };
        };
    };
}
