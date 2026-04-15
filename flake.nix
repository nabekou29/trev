{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        src = craneLib.cleanCargoSource ./.;

        commonArgs = {
          inherit src;
          strictDeps = true;
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux (
            with pkgs;
            [
              xorg.libX11
              xorg.libXcursor
              xorg.libXrandr
              xorg.libXi
              wayland
              libxkbcommon
            ]
          );
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        trev = craneLib.buildPackage (
          commonArgs
          // {
            # Tests depend on git and filesystem access unavailable in the nix sandbox.
            # Tests are run separately in CI.
            doCheck = false;
          }
        );
      in
      {
        packages.default = trev;

        devShells = {
          # Full development environment
          default = craneLib.devShell {
            inputsFrom = [ trev ];
            packages = with pkgs; [
              just
              cargo-release
              git-cliff
              cargo-llvm-cov
            ];
          };

          # Minimal CI environment
          ci = craneLib.devShell {
            inputsFrom = [ trev ];
            packages = with pkgs; [
              cargo-llvm-cov
            ];
          };
        };
      }
    );
}
