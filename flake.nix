{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
    nixpkgs-mozilla.url = "github:mozilla/nixpkgs-mozilla";
  };

  outputs = { self, nixpkgs, utils, naersk, nixpkgs-mozilla }:
    utils.lib.eachDefaultSystem (system:
      let
        rustDate = "2023-01-09";
        mozilla = import nixpkgs-mozilla;
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            mozilla
            (self: super: {
              rustc = (self.rustChannelOf { date = rustDate; channel = "nightly"; }).rust;
              cargo = (self.rustChannelOf { date = rustDate; channel = "nightly"; }).rust;
            })
          ];
        };
        naersk' = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = with pkgs;
          naersk'.buildPackage
            {
              src = ./.;
              nativeBuildInputs = [ pkg-config ];
              buildInputs = [ openssl ];
            };
        devShell = with pkgs; mkShell {
          nativeBuildInputs = [ pkg-config ];
          buildInputs = [ cargo rustc rustfmt pre-commit rustPackages.clippy openssl ];
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      });
}
