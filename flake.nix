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
            tree-sitter
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
