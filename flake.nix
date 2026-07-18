{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
        url = "github:oxalica/rust-overlay";
        inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ...}:
    flake-utils.lib.eachDefaultSystem ( system:
      let
        overlays = [ (import rust-overlay)];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "rust-analyzer" "rustfmt" "clippy" ];
            };
      in
      {
        devShells.default = pkgs.mkShell {
            nativeBuildInputs = [
                rustToolchain

                pkgs.pkg-config
                pkgs.gcc
                pkgs.just
                pkgs.pre-commit
                pkgs.cargo-nextest
                pkgs.yq
            ];
            buildInputs = [
                pkgs.openssl
            ];
        };
      });
}
