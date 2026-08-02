{
  description = "Blue Environment — Wayland desktop shell (Smithay + Tauri + Svelte)";

  # NOTE: like the Gentoo/Void files in this directory, this flake wraps
  # prebuilt release binaries with `stdenv.mkDerivation` (fetching a
  # release tarball) rather than building from source via
  # `rustPlatform.buildRustPackage` + `buildNpmPackage`, because a
  # from-source Nix build needs a `cargoHash`/`npmDepsHash` pinned against
  # a committed Cargo.lock/package-lock.json (neither is currently
  # committed upstream — see ROADMAP.md). Swapping this for a from-source
  # derivation once those lockfiles exist is straightforward and is the
  # preferred long-term approach for a Nix/NixOS package.

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      packages = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.stdenv.mkDerivation rec {
            pname = "blue-environment";
            version = "@VERSION@";

            src = pkgs.fetchurl {
              url = "https://github.com/HackerOS-Linux-System/Blue-Environment/releases/download/v${version}/blue-environment-${version}-${system}.tar.gz";
              # `nix-prefetch-url` this against a real release before use.
              sha256 = pkgs.lib.fakeSha256;
            };

            nativeBuildInputs = [ pkgs.autoPatchelfHook ];
            buildInputs = [
              pkgs.libseat
              pkgs.libGL
              pkgs.wayland
              pkgs.xwayland
              pkgs.ruby
            ];

            unpackPhase = "tar xzf $src";

            installPhase = ''
              mkdir -p $out/share/Blue-Environment/lib $out/bin
              install -m755 blue-environment $out/share/Blue-Environment/blue-environment
              install -m755 blue-compositor $out/share/Blue-Environment/lib/blue-compositor
              install -m755 blue $out/bin/blue

              mkdir -p $out/share/wayland-sessions $out/share/applications
              cat > $out/share/wayland-sessions/blue-environment.desktop <<DESK
[Desktop Entry]
Name=Blue Environment
Exec=$out/share/Blue-Environment/lib/blue-compositor
Type=Application
DesktopNames=Blue
DESK
              cat > $out/share/applications/blue-environment.desktop <<DESK
[Desktop Entry]
Name=Blue Environment
Exec=$out/share/Blue-Environment/blue-environment
Icon=blue-environment
Type=Application
Categories=System;
DESK
            '';

            meta = with pkgs.lib; {
              description = "Blue Environment Wayland desktop shell";
              homepage = "https://github.com/HackerOS-Linux-System/Blue-Environment";
              license = licenses.gpl3Only;
              platforms = systems;
            };
          };
        });
    };
}
