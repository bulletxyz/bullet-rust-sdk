{
  description = "bullet-rust-sdk development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
            targets = [ "wasm32-unknown-unknown" ];
          };

          darwinDeps = pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];
        in
        {
          default = pkgs.mkShell {
            name = "bullet-rust-sdk-dev";

            packages = [
              rust
              pkgs.cargo-nextest
              pkgs.just
              pkgs.wasm-pack
              pkgs.pkg-config
              pkgs.openssl
            ] ++ darwinDeps;

            shellHook = ''
              export OPENSSL_NO_VENDOR=1
              export CARGO_NET_GIT_FETCH_WITH_CLI=true
            '';
          };
        }
      );
    };
}
