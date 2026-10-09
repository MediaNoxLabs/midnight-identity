{ rust-bin, rust-overlay }:

let
  nightlyVersion = "2026-03-18";
  rustOverrideArgs = {
    extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" "llvm-tools" ];
    targets    = [ "wasm32-unknown-unknown" "aarch64-apple-ios-sim" ];
  };
in
{
  rust =
    rust-bin.nightly.${nightlyVersion}.default.override rustOverrideArgs;
}
