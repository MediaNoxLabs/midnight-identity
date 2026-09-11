{
  description = "Midnight DID — native Rust implementation";

  nixConfig = {
    extra-substituters     = [ "https://cache.iog.io" ];
    extra-trusted-public-keys = [ "hydra.iohk.io:f/Ea+s+dFdN+3Y/G+FDgSq+a5NEWhJGzdjvKNGv0/EQ=" ];
  };

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url  = "github:hercules-ci/flake-parts";
    # Repo moved yshyn-iohk -> MediaNoxLabs. Pinned by rev rather than by branch
    # tip: `dioxus-vc-demo` is a moving prototype branch, and this rev is the one
    # that was already locked, so the repin is content-identical.
    # rev 591a3170 is the tip of `dioxus-vc-demo` as of 2026-05-28.
    midnight-ledger = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-ledger";
      rev   = "591a31700f8956744227211b96f5594a91400388";
      flake = false;
    };
    # midnight-zk fork providing the patched `midnight-proofs` crate that
    # `midnight-ledger`'s `transient-crypto` references via [patch.crates-io].
    # Without this, `cargo build -p midnight-did-runtime` fails on the
    # ParamsKZG::{read_mmap_arc, write_mmap_companion, read_custom_lazy}
    # methods that only exist on the patched fork. See ADR 0006.
    # Pinned by rev for the same reason as midnight-ledger above. Here it is load
    # bearing: `feat/v0.7-h-poly-streaming` has since advanced to aeeff63c, while
    # Cargo.toml's [patch.crates-io] pins cf60e3cc — the two must agree or the
    # flake-materialised source and the cargo-built crate diverge.
    # rev cf60e3cc is the tip of `feat/v0.7-h-poly-streaming` as of 2026-05-23.
    midnight-zk = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-zk";
      rev   = "cf60e3ccb87f2de40b1307f7e78abdcb4c696c91";
      flake = false;
    };
    compact = {
      url = "github:MediaNoxLabs/compact/codegen-rust";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, flake-parts, ... }@inputs:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];

      imports = [
        ./nix/devShells.nix
        ./nix/overlays.nix
        ./nix/compact.nix
      ];

      perSystem =
        { system, ... }:
        {
          _module.args = {
            inherit rust-overlay;
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            };
            midnightDidRsLib = {
              rustTools = import ./nix/rustTools.nix {
                rust-bin     = (import nixpkgs { inherit system; overlays = [ (import rust-overlay) ]; }).rust-bin;
                rust-overlay = rust-overlay;
              };
              sources = {
                midnight-ledger = inputs.midnight-ledger;
                midnight-zk     = inputs.midnight-zk;
              };
            };
          };
        };
    };
}
