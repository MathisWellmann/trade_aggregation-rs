{
  description = "Flake for trade_aggregation-rs";

  inputs = {
    nixpks.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rust = (
          pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
            targets = ["x86_64-unknown-linux-gnu"];
          }
        );
        buildInputs = with pkgs; [
          openssl
          protobuf
          clang
          pkg-config
          fontconfig
          cmake
          # We use some `rustfmt` rules that are only available on the nightly channel.
          (lib.hiPrio rust-bin.nightly."2026-02-01".rustfmt)
          rust
        ];
        rust_tools = with pkgs; [
          taplo
          cargo-semver-checks
        ];
        nix_tools = with pkgs; [
          alejandra # Nix code formatter.
          deadnix # Nix dead code checker.
          statix # Nix static code checker.
        ];
      in
        with pkgs; {
          devShells.default = mkShell {
            buildInputs = buildInputs ++ nix_tools ++ rust_tools;
          };
        }
    );
}
