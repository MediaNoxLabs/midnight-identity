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
    # The MAINTAINED ledger-8 line, `ledger-8-patched` (upstream `ledger-8` plus
    # the curated prover patches), pinned by rev. Until 2026-09-15 this pointed
    # at the `dioxus-vc-demo` prototype branch (591a3170); the maintained line
    # is what the branch scheme says a consumer should pin, and the swap was
    # proven first: check, test (48 suites) and the wasm build all green.
    # rev 16b451e7 is the head of `ledger-8-patched` as of 2026-09-15.
    midnight-ledger = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-ledger";
      rev   = "16b451e7da024d4fae3c668e13f1d7041c30cd5f";
      flake = false;
    };
    # midnight-zk fork providing the patched `midnight-proofs` crate that
    # `midnight-ledger`'s `transient-crypto` references via [patch.crates-io].
    # Without this, `cargo build -p midnight-did-runtime` fails on the
    # ParamsKZG::{read_mmap_arc, write_mmap_companion, read_custom_lazy}
    # methods that only exist on the patched fork. See ADR 0006.
    # The MAINTAINED 0.7 zk line, `proofs-0.7-patched` (the consumer line for
    # ledger-8), pinned by rev — the same rev `ledger-8-patched` itself pins.
    # This pin is load bearing: Cargo.toml's [patch.crates-io] must name the
    # same rev or the flake-materialised source and the cargo-built crate
    # diverge. Until 2026-09-15 this was the pre-scheme `feat/v0.7-h-poly-
    # streaming` branch at cf60e3cc.
    # rev 083c8282 is the head of `proofs-0.7-patched` as of 2026-09-15.
    midnight-zk = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "midnight-zk";
      rev   = "083c82824dc5979fd7d509229e6f6b362d5b1ebf";
      flake = false;
    };
    compact = {
      type  = "github";
      owner = "MediaNoxLabs";
      repo  = "compact";
      rev   = "611a3dda0c512326a2c15a2e68ce20f8dcf58105";
      # The compiler revision's newer Nixpkgs pin fails while building
      # compiler-rt on aarch64-darwin. Keep the last repository-validated pin
      # until that upstream Darwin regression is fixed.
      inputs.nixpkgs.url = "github:NixOS/nixpkgs/bcc4a9d9533c033d806a46b37dc444f9b0da49dd";
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
