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
            # Node.js LTS — the runtime the browser MCP server
            # (chrome-devtools-mcp, run via npx) needs. Bundling it
            # here keeps the MCP runtime self-contained in mAId: the
            # launcher enters this flake, so node need not be on the
            # user's PATH.
            pkgs.nodejs_22
          ];
        };
      }
    );
}
