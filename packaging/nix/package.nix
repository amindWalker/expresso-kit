# Nix package for expresso-kit
# This file can be used standalone or submitted to nixpkgs
#
# Usage (standalone with flakes):
#   nix build .#expresso-kit
#   nix run .#expresso-kit
#
# Usage (with nix-shell):
#   nix-shell -p '(import ./packaging/nix/package.nix { })'
#
# For nixpkgs PR, copy this to:
#   pkgs/by-name/ex/expresso-kit/package.nix

{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  installShellFiles,
  stdenv,
  darwin,
  # Optional dependencies for full functionality
  git,
  docker,
  # Nix-specific options
  nix-update-script,
}:

rustPlatform.buildRustPackage rec {
  pname = "expresso-kit";
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "amindWalker";
    repo = "expresso-kit";
    rev = "v${version}";
    hash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="; # Replace after first build
  };

  cargoLock = {
    lockFile = "${src}/Cargo.lock";
    # Uncomment if any dependencies use git sources:
    # outputHashes = {
    #   "some-crate-0.1.0" = "sha256-...";
    # };
  };

  nativeBuildInputs = [
    pkg-config
    installShellFiles
  ];

  buildInputs = lib.optionals stdenv.hostPlatform.isDarwin [
    darwin.apple_sdk.frameworks.Security
    darwin.apple_sdk.frameworks.SystemConfiguration
  ];

  # Build only the main binary
  cargoBuildFlags = [ "--package" "expresso-kit" ];

  # Skip tests that require network or docker
  cargoTestFlags = [ "--package" "expresso-kit" ];
  checkFlags = [
    # Skip integration tests that need external services
    "--skip=integration"
    "--skip=docker"
  ];

  # Environment variables for build
  env = {
    # Ensure reproducible builds
    VERGEN_IDEMPOTENT = "1";
  };

  postInstall = ''
    # Install shell completions if the binary supports them
    # Uncomment when shell completions are implemented:
    # installShellCompletion --cmd expresso-kit \
    #   --bash <($out/bin/expresso-kit completions bash) \
    #   --fish <($out/bin/expresso-kit completions fish) \
    #   --zsh <($out/bin/expresso-kit completions zsh)

    # Install documentation
    install -Dm644 README.md -t $out/share/doc/${pname}/ || true
  '';

  passthru = {
    updateScript = nix-update-script { };
  };

  meta = {
    description = "Automated project validator for local/cloud environments with TUI and CLI";
    longDescription = ''
      EspressoKit is an automated project validator that validates repository
      configurations for local/cloud environments. It tests against CI deployment
      to guarantee idempotent and reproducible results, ensuring easier development
      and great developer experience.

      Features:
      - Interactive TUI (Terminal User Interface)
      - CLI for automation and scripting
      - Docker Compose validation
      - Environment file (.env) validation
      - GitHub Actions workflow generation
      - Cross-platform support (Linux, macOS)
    '';
    homepage = "https://github.com/amindWalker/expresso-kit";
    changelog = "https://github.com/amindWalker/expresso-kit/releases/tag/v${version}";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [
      amindWalker
    ];
    mainProgram = "expresso-kit";
    platforms = lib.platforms.unix;
  };
}
