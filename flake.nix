{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        runtimeDeps = with pkgs; [
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

        # Bundled fonts — only what ferritebar actually uses
        barFonts = pkgs.symlinkJoin {
          name = "ferritebar-fonts";
          paths = [
            pkgs.font-awesome
            pkgs.fira-sans
          ];
        };

        barFontconfig = pkgs.writeText "ferritebar-fonts.conf" ''
          <?xml version="1.0"?>
          <!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
          <fontconfig>
            <dir>${barFonts}/share/fonts</dir>
            <cachedir>/tmp/ferritebar-fontcache</cachedir>
            <rescan><int>0</int></rescan>
          </fontconfig>
        '';

        unwrapped = pkgs.rustPlatform.buildRustPackage {
          pname = "ferritebar-unwrapped";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = runtimeDeps;
        };
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

          buildInputs = runtimeDeps;

          shellHook = ''
            export RUST_LOG=ferritebar=debug
          '';
        };

        packages.default = pkgs.symlinkJoin {
          name = "ferritebar";
          paths = [ unwrapped ];
          nativeBuildInputs = [ pkgs.makeWrapper ];
          postBuild = ''
            wrapProgram $out/bin/ferritebar \
              --set GSK_RENDERER cairo \
              --set GDK_DISABLE "vulkan,gl,dmabuf,offload" \
              --set GTK_A11Y none \
              --set NO_AT_BRIDGE 1 \
              --set GDK_BACKEND wayland \
              --set GTK_MEDIA none \
              --set GTK_CSD 0 \
              --unset GTK_IM_MODULE \
              --set FONTCONFIG_FILE ${barFontconfig}
          '';
        };
      });
}
