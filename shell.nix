# Development shell for non-flake users
# Usage: nix-shell
#
# This provides a development environment with all necessary tools

{ pkgs ? import <nixpkgs> { } }:

pkgs.mkShell {
  name = "expresso-kit-dev";

  buildInputs = with pkgs; [
    # Rust toolchain
    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer

    # Build dependencies
    pkg-config
    openssl

    # Development tools
    git
    docker
    docker-compose

    # For shell completions
    installShellFiles
  ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
    pkgs.darwin.apple_sdk.frameworks.Security
    pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
    pkgs.libiconv
  ];

  shellHook = ''
    echo "🦀 Expresso-Kit Development Shell (legacy nix-shell)"
    echo ""
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    echo ""
    echo "Commands:"
    echo "  cargo build    - Build the project"
    echo "  cargo test     - Run tests"
    echo "  cargo run      - Run the application"
    echo ""
    echo "For flakes-enabled Nix, use: nix develop"
  '';

  # Ensure Cargo uses the system OpenSSL
  OPENSSL_DIR = "${pkgs.openssl.dev}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
}
