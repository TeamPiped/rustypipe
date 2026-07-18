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
                pkgs.cmake
                pkgs.just
                pkgs.pre-commit
                pkgs.cargo-nextest
                pkgs.yq
                pkgs.direnv
            ];
            buildInputs = [
                pkgs.openssl
            ];
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            BINDGEN_EXTRA_CLANG_ARGS = with pkgs; builtins.concatStringsSep " " [
                "-isystem ${stdenv.cc.cc}/include/c++/${lib.getVersion stdenv.cc.cc}"
                "-isystem ${stdenv.cc.cc}/include/c++/${lib.getVersion stdenv.cc.cc}/${stdenv.hostPlatform.config}"
                "-idirafter ${stdenv.cc.cc.lib}/lib/clang/${lib.getVersion stdenv.cc.cc}/include"
                "-idirafter ${glibc.dev}/include"
              ];
            shellHook = ''
                export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
              '';
        };
      });
}
