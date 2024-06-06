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
          pkgs.rust-bin.stable."1.77.0".default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
            targets = ["x86_64-unknown-linux-gnu"];
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
              # We use some `rustfmt` rules that are only available on the nightly channel.
              (lib.hiPrio rust-bin.nightly."2024-04-01".rustfmt)
              rust
              taplo
              cargo-semver-checks
            ];
          };
        }
    );
}
