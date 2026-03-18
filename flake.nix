{
  description = "bullet-rust-sdk development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      wasmPackVersion = "0.14.0";

      # Pre-built wasm-pack binaries per system (nixpkgs lags behind releases)
      wasmPackSources = {
        "aarch64-darwin" = {
          url = "https://github.com/drager/wasm-pack/releases/download/v${wasmPackVersion}/wasm-pack-v${wasmPackVersion}-aarch64-apple-darwin.tar.gz";
          hash = "sha256-nQ5wxrIp3hjwq/6RDylj6PCeuuIYJQ6bCaHD/dlVvvk=";
          bin = "wasm-pack-v${wasmPackVersion}-aarch64-apple-darwin/wasm-pack";
        };
        "x86_64-linux" = {
          url = "https://github.com/drager/wasm-pack/releases/download/v${wasmPackVersion}/wasm-pack-v${wasmPackVersion}-x86_64-unknown-linux-musl.tar.gz";
          hash = "sha256-J4qNZoCFgh9NGmN72GTxcT+HKwrjoRjHdWKjCMCr/o0=";
          bin = "wasm-pack-v${wasmPackVersion}-x86_64-unknown-linux-musl/wasm-pack";
        };
        "aarch64-linux" = {
          url = "https://github.com/drager/wasm-pack/releases/download/v${wasmPackVersion}/wasm-pack-v${wasmPackVersion}-aarch64-unknown-linux-musl.tar.gz";
          hash = "sha256-WUHHsFBgRA/zfuUP6QCaQI5j+lumB6Owc29aiH7F8so=";
          bin = "wasm-pack-v${wasmPackVersion}-aarch64-unknown-linux-musl/wasm-pack";
        };
      };

      makeWasmPack =
        pkgs:
        let
          src = wasmPackSources.${pkgs.system};
        in
        pkgs.stdenvNoCC.mkDerivation {
          pname = "wasm-pack";
          version = wasmPackVersion;
          src = pkgs.fetchurl { inherit (src) url hash; };
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.autoPatchelfHook ];
          installPhase = ''
            install -Dm755 wasm-pack $out/bin/wasm-pack
          '';
        };
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rust = pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
              "clippy"
              "rustfmt"
            ];
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
              (makeWasmPack pkgs)
              pkgs.pkg-config
              pkgs.openssl
            ]
            ++ darwinDeps;

            shellHook = ''
              export OPENSSL_NO_VENDOR=1
              export CARGO_NET_GIT_FETCH_WITH_CLI=true
            '';
          };
        }
      );
    };
}
