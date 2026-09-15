{ ... }:
{
  perSystem =
    {
      pkgs,
      midnightDidRsLib,
      midnightLedgerSrc,
      midnightZkSrc,
      compactRuntimeRsSrc,
      compactRuntimeRsMacrosSrc,
      compactPkg,
      ...
    }:
    let
      inherit (midnightDidRsLib.rustTools) rust;

      factoryPackages = with pkgs; [
        git
        gh
        nodejs_24
      ];

      rustPackages = with pkgs; [
        rust
        just
        taplo
        cargo-nextest
        cargo-llvm-cov
        jq
      ] ++ factoryPackages;

      factoryShellHook = ''
        export ROOT_DIR=$(${pkgs.git}/bin/git rev-parse --show-toplevel)
        cd "$ROOT_DIR"
      '';

      workspaceShellHook = ''
        export ROOT_DIR=$(${pkgs.git}/bin/git rev-parse --show-toplevel)
        cd "$ROOT_DIR"

        # Materialize third_party/midnight-ledger as a symlink to the nix-store path.
        TARGET="${midnightLedgerSrc}"
        LINK="$ROOT_DIR/third_party/midnight-ledger"
        mkdir -p "$ROOT_DIR/third_party"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        # Materialize third_party/midnight-zk as a symlink to the nix-store path.
        # Provides the patched `midnight-proofs` crate referenced by
        # [patch.crates-io] in the root Cargo.toml. See ADR 0006.
        TARGET="${midnightZkSrc}"
        LINK="$ROOT_DIR/third_party/midnight-zk"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        # Materialise compact's runtime-rs + runtime-rs-macros subtrees inside
        # third_party/compact/, mirroring the in-repo layout so that the
        # relative path `../runtime-rs-macros` (in midnight-compact-runtime's Cargo.toml)
        # and `../runtime-rs` (in runtime-rs-macros' dev-deps) both resolve
        # correctly without aliasing.
        mkdir -p "$ROOT_DIR/third_party/compact"

        TARGET="${compactRuntimeRsSrc}"
        LINK="$ROOT_DIR/third_party/compact/runtime-rs"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        TARGET="${compactRuntimeRsMacrosSrc}"
        LINK="$ROOT_DIR/third_party/compact/runtime-rs-macros"
        if [ -L "$LINK" ] && [ "$(readlink "$LINK")" = "$TARGET" ]; then
          :
        else
          rm -rf "$LINK"
          ln -s "$TARGET" "$LINK"
          echo "Linked $LINK -> $TARGET"
        fi

        echo "Entered midnight-identity devshell. Run 'just --list' for available commands."
      '';

      mkWorkspaceShell = packages: pkgs.mkShell {
        inherit packages;
        shellHook = workspaceShellHook;
        env.RUST_LOG = "info";
      };
    in
    {
      devShells = {
        # Fast policy shell used by bootstrap and repository-local Git hooks.
        factory = pkgs.mkShell {
          packages = factoryPackages;
          shellHook = factoryShellHook;
        };

        # Lightweight Pi shell for factory supervision. Keep Compact/Rust
        # materialization on the explicit default/rust shells so `--pi` can
        # start without constructing unrelated toolchains.
        pi = pkgs.mkShell {
          packages = [ pkgs.pi-coding-agent ] ++ factoryPackages;
          shellHook = factoryShellHook;
        };

        # Interactive factory shell: keep compactc and pi.dev available.
        default = mkWorkspaceShell (
          [
            compactPkg
            pkgs.pi-coding-agent
          ]
          ++ rustPackages
        );

        # Ordinary Rust gates do not need to construct compactc and its
        # proving/Node dependency closure.
        rust = mkWorkspaceShell rustPackages;
      };
    };
}
