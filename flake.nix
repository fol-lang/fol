{
  description = "FOL development shell";

  inputs = {
    # Pinned to a release branch so the toolchain around Rust (tree-sitter,
    # mdbook, gcc) does not drift under the pinned compiler.
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };
        # One source of truth for the Rust version: the same file CI and local
        # rustup read.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # fol-editor regenerates the grammar with this CLI and then asserts
        # against the bytes it produces, so the version is an exact pin rather
        # than a floor -- nixpkgs ships 0.25.3, which emits an older parser ABI
        # and fails 33 of those tests. Same version CI installs, and the one
        # `fol tool` refuses to run without.
        treeSitterVersion = "0.26.8";
        treeSitterAsset = {
          x86_64-linux = {
            name = "linux-x64";
            hash = "sha256-l1SjKADwuXAVJ4LfF3tKR8cR405lGnrOs4TYvSn6E24=";
          };
          aarch64-linux = {
            name = "linux-arm64";
            hash = "sha256-4znWUzsggw3RZm/jIK/4XTAbP1mWSjg2hwt39IJ/mhc=";
          };
        }.${system} or (throw "FOL is linux-only; no tree-sitter CLI pinned for ${system}");
        treeSitterCli = pkgs.stdenv.mkDerivation {
          pname = "tree-sitter";
          version = treeSitterVersion;
          src = pkgs.fetchurl {
            url = "https://github.com/tree-sitter/tree-sitter/releases/download/"
              + "v${treeSitterVersion}/tree-sitter-${treeSitterAsset.name}.gz";
            inherit (treeSitterAsset) hash;
          };
          dontUnpack = true;
          nativeBuildInputs = [ pkgs.autoPatchelfHook pkgs.gzip ];
          buildInputs = [ pkgs.stdenv.cc.cc.lib ];
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/bin"
            gzip -dc "$src" > "$out/bin/tree-sitter"
            chmod +x "$out/bin/tree-sitter"
            runHook postInstall
          '';
        };
      in
      {
        devShells.default = pkgs.mkShell {
          strictDeps = true;

          # `make verify` needs more than a Rust compiler: build scripts need a
          # C toolchain, fol-editor's tests shell out to the tree-sitter CLI,
          # `make docs` needs mdbook, and the H7 interop smoke needs gcc.
          packages = with pkgs; [
            rustToolchain
            rust-analyzer
            llvmPackages.lldb
            gcc
            binutils
            gnumake
            pkg-config
            git
            curl
            treeSitterCli
            mdbook
          ];

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            export PATH="$PATH:$PWD:$PWD/target/debug:$PWD/target/release"
            # The H7 interop smoke hands this compiler to LINC's ABI probe, and
            # it must be the real binary rather than the nix wrapper script.
            export FOL_H7_GCC="${pkgs.gcc.cc}/bin/gcc"
          '';
        };
      });
}
