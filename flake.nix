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
        # One source of truth for the Rust version: this flake. Nothing reads a
        # rustup toolchain file, so a checkout that never installs rustup builds
        # exactly what CI builds.
        rustVersion = "1.89.0";
        rustToolchain = pkgs.rust-bin.stable.${rustVersion}.default.override {
          extensions = [ "rust-src" "rustfmt" "clippy" ];
          # Release binaries are static musl so one artifact runs on any Linux.
          targets = [ "x86_64-unknown-linux-musl" "aarch64-unknown-linux-musl" ];
        };

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

        # `make verify` needs more than a Rust compiler: build scripts need a
        # C toolchain, fol-editor's tests shell out to the tree-sitter CLI,
        # `make docs` needs mdbook, and the H7 interop smoke needs gcc.
        commonPackages = with pkgs; [
          rustToolchain
          rust-analyzer
          llvmPackages.lldb
          gcc
          # clang is a promoted interop compiler family, so the smoke and LINC
          # own clang observation path need a real one on PATH.
          clang
          binutils
          gnumake
          pkg-config
          git
          curl
          treeSitterCli
          mdbook
        ];

        # Release binaries link against musl so one artifact runs on any Linux.
        # fol-editor's build script compiles the generated `parser.c`, so this
        # needs a musl C toolchain and not only the Rust target.
        muslTarget = {
          x86_64-linux = "x86_64-unknown-linux-musl";
          aarch64-linux = "aarch64-unknown-linux-musl";
        }.${system} or (throw "no musl release target pinned for ${system}");
        muslCc = "${pkgs.pkgsMusl.stdenv.cc}/bin/cc";
        # Cargo spells its per-target overrides with an upper-snake triple.
        muslKey = builtins.replaceStrings [ "-" ] [ "_" ] (pkgs.lib.toUpper muslTarget);
        # One source of truth for the package version: the workspace manifest.
        # Parsed rather than duplicated, so a release cannot ship a package
        # whose version disagrees with the crate it built.
        folVersion = (builtins.fromTOML (builtins.readFile ./Cargo.toml)).workspace.package.version;

        # The installable FOL toolchain.
        #
        # An installed toolchain finds its payloads next to the running binary:
        # `<bin>/std` for the standard library and `<bin>/runtime` for the
        # runtime crate the backend compiles against. That is the layout
        # `fol self install` produces, so a nix-installed compiler resolves
        # packages exactly the way a self-installed one does.
        # Built with the pinned toolchain rather than nixpkgs' default rustc:
        # the interop crates require 1.89, and a package that silently used a
        # different compiler than `make verify` did would not be the same build.
        folRustPlatform = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        folPackage = folRustPlatform.buildRustPackage {
          pname = "fol";
          version = folVersion;
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
            # The interop stack is pinned by git revision rather than published
            # to crates.io, so each git source needs its own fixed-output hash.
            outputHashes = {
              "follang-gerc-0.1.0" = "sha256-XHAQKb4T3qaQeqAmmy1XxgVIvXdNbGTDL4ZeYPaVoNY=";
              "follang-linc-0.1.0" = "sha256-ccP8WlRkmTY13H1GWiXPYe70xkK53jDoOGg6eMb0VMk=";
              "follang-parc-0.16.0" = "sha256-2XC1dyvnFGCUyV5ZbiirLR0l5jpru4VSwhKzFtBYnMU=";
            };
          };

          # Only the two user-facing binaries. Building the whole workspace
          # would also build every test-only target for no benefit here.
          cargoBuildFlags = [ "--bin" "folc" "--bin" "fol" ];

          # The suite drives rustc, a C toolchain, tree-sitter, and real
          # filesystem fixtures. `make verify` is that gate; a package build is
          # not the place for it.
          doCheck = false;

          # `fol-editor`'s build script regenerates the grammar, so the pinned
          # tree-sitter CLI is a build dependency rather than a dev convenience.
          nativeBuildInputs = [ pkgs.makeWrapper treeSitterCli ];

          postInstall = ''
            # The standard library, as the compiler's bundled-std lookup wants
            # it: the `std` package itself, not the store root above it.
            mkdir -p "$out/bin/std"
            cp -r lang/library/std/. "$out/bin/std/"

            # The runtime crate source. The backend compiles this with rustc on
            # every FOL build, so it ships as source rather than as an rlib.
            mkdir -p "$out/bin/runtime"
            cp -r lang/execution/fol-runtime/. "$out/bin/runtime/"

            # A FOL build shells out to rustc and a linker. Wrapping them in
            # means `nix run` works without the caller assembling a toolchain.
            for binary in folc fol; do
              wrapProgram "$out/bin/$binary" \
                --prefix PATH : "${pkgs.lib.makeBinPath [ rustToolchain pkgs.gcc ]}"
            done
          '';

          meta = with pkgs.lib; {
            description = "The FOL programming language compiler and toolchain";
            homepage = "https://github.com/fol-lang/fol";
            mainProgram = "folc";
            platforms = platforms.linux;
          };
        };
      in
      {
        packages.default = folPackage;
        packages.fol = folPackage;

        # `nix run github:fol-lang/fol` compiles a FOL package; `.#fol` is the
        # toolchain manager.
        apps.default = {
          type = "app";
          program = "${folPackage}/bin/folc";
        };
        apps.folc = self.apps.${system}.default;
        apps.fol = {
          type = "app";
          program = "${folPackage}/bin/fol";
        };

        # `nix build .#checks.<system>.package` builds the toolchain; the real
        # gate is `make verify` inside `nix develop`, which needs a writable
        # tree and network-free fixtures a sandboxed check cannot provide.
        checks.package = folPackage;

        devShells.default = pkgs.mkShell {
          strictDeps = true;
          packages = commonPackages;

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            export PATH="$PATH:$PWD:$PWD/target/debug:$PWD/target/release"
            # The H7 interop smoke hands this compiler to LINC's ABI probe, and
            # it must be the real binary rather than the nix wrapper script.
            export FOL_H7_GCC="${pkgs.gcc.cc}/bin/gcc"
            # musl is a separately promoted interop platform, so the smoke
            # needs a real musl compiler as well as the glibc one.
            export FOL_H7_MUSL_CC="${muslCc}"
            export FOL_H7_CLANG="${pkgs.clang}/bin/clang"
          '';
        };

        # `nix develop .#release` is what the release workflow builds in.
        devShells.release = pkgs.mkShell ({
          strictDeps = true;
          packages = commonPackages;

          CARGO_BUILD_TARGET = muslTarget;
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        } // {
          "CC_${builtins.replaceStrings [ "-" ] [ "_" ] muslTarget}" = muslCc;
          "CARGO_TARGET_${muslKey}_LINKER" = muslCc;
        });
      });
}
