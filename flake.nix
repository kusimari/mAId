{
  description = "mAId — repo-local rust toolchain, just, and the Node.js runtime the browser MCP server needs (self-contained env for all workspace members)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
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
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" "rust-analyzer" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.just
            # The browser MCP server runs on Node; bundling it here
            # (vs. the user's PATH) is what keeps that runtime
            # self-contained — the launcher enters this flake to reach it.
            pkgs.nodejs_22
          ];
        };
      }
    );
}
