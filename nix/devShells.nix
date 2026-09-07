{ ... }:
{
  perSystem =
    { pkgs, midnightDidRsLib, midnightLedgerSrc, midnightZkSrc, compactRuntimeRsSrc, compactRuntimeRsMacrosSrc, factoryComponentsSrc, compactPkg, ... }:
    let
      inherit (midnightDidRsLib.rustTools) rust;
    in
    {
      devShells.default = pkgs.mkShell {
        packages = [ compactPkg ] ++ (with pkgs; [
          rust
          just
          taplo
          cargo-nextest
          cargo-llvm-cov
          git
          jq
          # Optional pi.dev operator shell (see doc/pi-development.md).
          # nix provides the binary; .pi/settings.json pins the
          # dev-loops extension it loads.
          pi-coding-agent
        ]);

        shellHook = ''
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
          # relative path `../runtime-rs-macros` (in compact-runtime's Cargo.toml)
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

          # Mount patextreme/ptah's source-only factory-components library at
          # .ptah/libs. Its native components/ and std/ layout is preserved,
          # so workflows can require ../../libs/components/<name>/component.
          if [ -e "$ROOT_DIR/.ptah/libs" ] && [ ! -L "$ROOT_DIR/.ptah/libs" ]; then
            echo "not replacing .ptah/libs: exists and is not a symlink — move it aside and re-enter" >&2
            exit 1
          fi
          mkdir -p "$ROOT_DIR/.ptah"
          ln -sfn "${factoryComponentsSrc}" "$ROOT_DIR/.ptah/libs"

          echo "Entered midnight-identity devshell. Run 'just --list' for available commands."
        '';

        env = {
          RUST_LOG = "info";
        };
      };
    };
}
