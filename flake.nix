{
  description = "mirufm - a GUI file explorer";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        runtimeLibs = with pkgs; [
          vulkan-loader
          wayland
          libxkbcommon
          libGL
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libxcb
          fontconfig
          freetype
        ];
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
      in {
        packages.default = rustPlatform.buildRustPackage {
          pname = "mirufm";
          version = "0.0.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            outputHashes = {
              "gpui-0.2.2" = "sha256-A4z6PwnAFOuzP6XymyLzOMwkDlxE3DpkSduyydlOUt8=";
              "zed-font-kit-0.14.1-zed" = "sha256-KXygi0olNQi5yM8eaJVykNDtbPMDjT+cWPBF8UrtXR4=";
              "zed-scap-0.0.8-zed" = "sha256-BihiQHlal/eRsktyf0GI3aSWsUCW7WcICMsC2Xvb7kw=";
              "xim-ctext-0.3.0" = "sha256-pRT4Sz1JU9ros47/7pmIW9kosWOGMOItcnNd+VrvnpE=";
            };
          };

          nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];
          buildInputs = runtimeLibs;

          cargoBuildFlags = [ "-p" "mirufm" ];
          doCheck = false;

          postInstall = ''
            wrapProgram $out/bin/mirufm \
              --set LD_LIBRARY_PATH ${pkgs.lib.makeLibraryPath runtimeLibs}
            install -Dm444 ${./assets/mirufm.desktop} $out/share/applications/mirufm.desktop
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ rust ] ++ runtimeLibs;
          nativeBuildInputs = [ pkgs.pkg-config ];
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
        };
      });
}
