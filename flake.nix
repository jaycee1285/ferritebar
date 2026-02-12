{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            pkg-config
          ];

          buildInputs = with pkgs; [
            gtk4
            gtk4-layer-shell
            glib
            cairo
            pango
            gdk-pixbuf
            graphene
            wayland
            wayland-protocols
            libpulseaudio
            libxkbcommon
          ];

          shellHook = ''
            export RUST_LOG=ferritebar=debug
          '';
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "ferritebar";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            gtk4
            gtk4-layer-shell
            glib
            cairo
            pango
            gdk-pixbuf
            graphene
            wayland
            wayland-protocols
            libpulseaudio
            libxkbcommon
          ];
        };
      });
}
