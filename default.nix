# Legacy Nix expression for non-flake users
# Usage:
#   nix-build
#   nix-shell
#   nix-env -if .
#
# Or with nixpkgs:
#   nix-build -E 'with import <nixpkgs> {}; callPackage ./packaging/nix/package.nix {}'

{ pkgs ? import <nixpkgs> { } }:

pkgs.callPackage ./packaging/nix/package.nix { }
