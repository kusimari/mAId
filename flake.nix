{
  description = "mAId - the dev shell (rust toolchain, just), the pinned runtimes, and the plain and ghost packages each built by its own Rust install";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # The memory MCP server, pinned to its published commit.
    pensieve = {
      url = "github:kusimari/pensieve/4ff98e71e442a8c5cccdb34bbab45a2c89786a04";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, pensieve }:
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

        pensieveServer = pensieve.packages.${system}.default;

        rustPlatform = pkgs.makeRustPlatform { cargo = rustToolchain; rustc = rustToolchain; };
        fs = pkgs.lib.fileset;
        # The workspace's crates and its lock, and nothing they do not
        # compile: no resources, plugin assets or test data, so editing one
        # does not rebuild a binary.
        crates = fs.toSource {
          root = ./.;
          fileset = fs.unions [
            ./Cargo.toml
            ./Cargo.lock
            ./ghost-in-shell/ghost/Cargo.toml
            ./ghost-in-shell/ghost/src
            ./ghost-in-shell/infusion/Cargo.toml
            ./ghost-in-shell/infusion/src
            ./ghost-in-shell/infusion/fixtures
            ./ghost-in-shell/shell/claude/Cargo.toml
            ./ghost-in-shell/shell/claude/src
            ./ghost-in-shell/tests/Cargo.toml
            ./ghost-in-shell/tests/src
            ./plain/Cargo.toml
            ./plain/src
            ./kaimux/Cargo.toml
            ./kaimux/src
          ];
        };
        # One cargo build per binary; `just test` runs the tests.
        bin = pname: flags: rustPlatform.buildRustPackage {
          inherit pname;
          version = "0.1.0";
          src = crates;
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = flags;
          doCheck = false;
        };
        plain-bin = bin "plain" [ "-p" "plain" "--bin" "plain" ];
        ghost-bin = bin "ghost-claude" [ "-p" "shell-claude" "--bin" "ghost-claude" ];
      in
      {
        packages = {
          inherit chrome-devtools-mcp plain-bin ghost-bin;

          # Plain's package, as `just install` puts it in the mAId profile:
          # the plugin marketplace claude and codex install from, the skills
          # kiro and agy link at, the browser launcher and the runtimes.
          # Built by plain's own install; nix only gives it the inputs.
          default = pkgs.runCommand "maid" { } ''
            ${plain-bin}/bin/plain install --prefix $out --resources ${./resources} \
              --runtime pensieve=${pensieveServer}/bin/pensieve \
              --runtime chrome-devtools-mcp=${chrome-devtools-mcp}/bin/chrome-devtools-mcp
          '';

          # The ghost's package, as `just ghost install` puts it in the
          # ghost's profile: its plugin and the binary its hooks call.
          ghost = pkgs.runCommand "ghost" { } ''
            ${ghost-bin}/bin/ghost-claude install --prefix $out --exe ${ghost-bin}/bin/ghost-claude \
              --root ${fs.toSource { root = ./.; fileset = ./ghost-in-shell/shell/claude/plugin; }}
          '';
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [
            rustToolchain
            pkgs.just
            pkgs.jq # the install tests read the agents' JSON; install edits kiro's MCP config
          ];
          # The core's adaptation test writes a habit through the real
          # memory server, the one the profile ships.
          MAID_TEST_PENSIEVE = "${pensieveServer}/bin/pensieve";
        };
      }
    );
}
