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

        # The plugin's manifests. `@version@` is filled in by the build.
        description = "mAId's skills";
        manifest = name: value: pkgs.writeText name (builtins.toJSON value);
        claudeMarketplace = manifest "claude-marketplace.json" {
          name = "maid";
          owner.name = "mAId";
          inherit description;
          plugins = [ { name = "maid"; source = "./plugins/maid"; inherit description; } ];
        };
        codexMarketplace = manifest "codex-marketplace.json" {
          name = "maid";
          plugins = [ {
            name = "maid";
            source = { source = "local"; path = "./plugins/maid"; };
            policy.installation = "AVAILABLE";
          } ];
        };
        claudePlugin = manifest "claude-plugin.json" {
          name = "maid";
          version = "@version@";
          inherit description;
          author.name = "mAId";
        };
        codexPlugin = manifest "codex-plugin.json" {
          name = "maid";
          version = "@version@";
          inherit description;
          skills = "./skills/";
        };
      in
      {
        packages = {
          inherit chrome-devtools-mcp maid-browser-mcp;

          # What `just install` puts in the mAId profile: the plugin
          # marketplace claude and codex install from, and the skills
          # path build-tool's REGISTRY links kiro and agy at (the
          # plugin's own skills, so there is one copy).
          default = pkgs.runCommand "maid" { } ''
            m=$out/share/maid/marketplace
            p=$m/plugins/maid
            mkdir -p $m/.claude-plugin $m/.agents/plugins $p/.claude-plugin $p/.codex-plugin $out/bin
            cp -r ${./resources/content/skills} $p/skills
            cp ${claudeMarketplace} $m/.claude-plugin/marketplace.json
            cp ${codexMarketplace} $m/.agents/plugins/marketplace.json
            cp ${claudePlugin} $p/.claude-plugin/plugin.json
            cp ${codexPlugin} $p/.codex-plugin/plugin.json
            # Both agents cache a plugin by version, so it must change
            # exactly when the plugin's content does.
            hash=$(cd $p && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -c1-12)
            chmod u+w $p/.claude-plugin/plugin.json $p/.codex-plugin/plugin.json
            sed -i "s/@version@/1.0.0-$hash/" $p/.claude-plugin/plugin.json $p/.codex-plugin/plugin.json
            ln -s marketplace/plugins/maid/skills $out/share/maid/skills
            ln -s ${maid-browser-mcp}/bin/maid-browser-mcp $out/bin/maid-browser-mcp
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.just
            pkgs.jq # the install tests read the agents' JSON
          ];
        };
      }
    );
}
