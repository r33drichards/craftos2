{
  description = "CraftOS-PC + Rust MCP server (GPS/rednet multi-computer testing)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (sys: f nixpkgs.legacyPackages.${sys});
    in {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          # Rust toolchain (per "use nix for cargo") + the emulator's C/C++ deps.
          # GPS/rednet headless build needs only SDL2 + Poco + OpenSSL + lua; the
          # GUI/audio/PDF deps are disabled at ./configure time.
          packages = with pkgs; [
            rustc
            cargo
            rust-analyzer
            clang
            pkg-config
            gnumake
            SDL2
            poco
            openssl
            ncurses
            # darwin.cpp hard-includes <png++/png.hpp>; provide the headers even
            # though screenshots are compiled out (NO_PNG).
            libpng
            pngpp
          ];

          # Help ./configure + the linker find Poco/OpenSSL headers and libs.
          shellHook = ''
            export CRAFTOS_NIX=1
            echo "craftos2 dev shell: rustc $(rustc --version 2>/dev/null | cut -d' ' -f2), $(cargo --version 2>/dev/null)"
          '';
        };
      });
    };
}
