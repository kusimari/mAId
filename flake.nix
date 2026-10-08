{
  description = "mAId - the installable (skills + the runtimes they need) and the dev shell (rust toolchain, just)";

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

        # Not in nixpkgs. The npm tarball declares no dependencies (it
        # ships bundled), so it needs no npm install, only node.
        chrome-devtools-mcp = pkgs.stdenvNoCC.mkDerivation rec {
          pname = "chrome-devtools-mcp";
          version = "1.10.1";
          src = pkgs.fetchurl {
            url = "https://registry.npmjs.org/chrome-devtools-mcp/-/chrome-devtools-mcp-${version}.tgz";
            hash = "sha256-ASy89ugy1PZwna0MIde+8XCJ6Ure6c8zF50V6gqa3ys=";
          };
          nativeBuildInputs = [ pkgs.makeWrapper ];
          installPhase = ''
            mkdir -p $out/lib/${pname} $out/bin
            cp -r . $out/lib/${pname}
            makeWrapper ${pkgs.nodejs_22}/bin/node $out/bin/chrome-devtools-mcp \
              --add-flags $out/lib/${pname}/build/src/bin/chrome-devtools-mcp.js \
              --set CHROME_DEVTOOLS_MCP_NO_UPDATE_CHECKS 1
          '';
        };

        maid-browser-mcp = pkgs.writeShellApplication {
          name = "maid-browser-mcp";
          runtimeInputs = [ chrome-devtools-mcp ];
          text = builtins.readFile ./resources/browser/launch;
        };
      in
      {
        packages = {
          inherit chrome-devtools-mcp maid-browser-mcp;

          # What `just install` puts in the mAId profile. The skills path
          # is the one build-tool's REGISTRY links agents at.
          default = pkgs.runCommand "maid" { } ''
            mkdir -p $out/share/maid $out/bin
            cp -r ${./resources/content/skills} $out/share/maid/skills
            ln -s ${maid-browser-mcp}/bin/maid-browser-mcp $out/bin/maid-browser-mcp
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.just
          ];
        };
      }
    );
}
