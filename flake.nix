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
              "rustfmt"
              "clippy"
            ];
            targets = ["x86_64-unknown-linux-gnu" "wasm32-unknown-unknown"];
          }
        );
      in
        with pkgs; {
          devShells.default = mkShell {
            buildInputs = [
              openssl
              protobuf
              clang
              pkg-config
              fontconfig
              cmake
              rust
            ];
          };
        }
    );
}
