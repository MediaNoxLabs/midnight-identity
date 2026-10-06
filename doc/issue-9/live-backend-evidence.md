# Issue #9 LiveBackend evidence

SPDX-License-Identifier: Apache-2.0

## Implemented shape in this worktree

`midnight-did-runtime::LiveBackend` is dependency-injected. Mutating calls run:

1. decode the in-process `DidContractCall` command;
2. read live indexer state through `LedgerIndexerProvider`;
3. execute generated Rust `did.compact` through `GeneratedDidExecutor`;
4. extract Compact proof data into `DidPrePartitionContractCall`;
5. hand generated proof material to the wallet provider;
6. balance, prove via `/prove-tx`, sign, submit, wait for finality, and reconcile.

Deployment now has an explicit constructor path: `GeneratedDidExecutor::deployment_request()` returns `DidDeploymentRequest` carrying generated constructor proof material (`constructor_id`, initial/final query contexts, aligned input/output, ordered public transcript, and redacted private outputs), and `LiveBackend::submit_deployment()` sends it through the same provider pipeline.

## Compatibility matrix

| Area | Evidence |
| --- | --- |
| Ledger source | Root `flake.lock` `midnight-ledger` is `MediaNoxLabs/midnight-ledger@b85f5d8e503fd1d7a1b128bbc1d7156baf823a65`. |
| Compact source | Root `flake.lock` `compact` is `MediaNoxLabs/compact@e53a8e88d32c561b69b2ed5f2e7e4c19b4c93d35` (`compactc 0.31.124`, runtime `0.16.102`). |
| Generated contract | `just codegen` regenerated `crates/midnight-did-runtime/src/contract/generated.rs` from official DID source `third_party/midnight-did@42a8e4aca1c6043f7f2b73463fa3aa3cd3d48b06`, path `packages/contract/src/did.compact`. |
| Constructor proof data | Generated `initial_state` now returns `ConstructorProofData`; runtime maps it into `DidConstructorProofMaterial`. |
| Ledger8 prepartition conversion | `DidPrePartitionContractCall` and `DidConstructorProofMaterial` convert directly to `midnight_ledger::construct::PrePartitionContractCall<DefaultDB>` using the canonical `b85f5d8e...` Ledger graph. |
| Proof transport | `HttpProofProvider` posts only `BalancedDidTransaction` values constructed from typed `LedgerProveTxRequest` (`Transaction<Signature, ProofPreimageMarker, PedersenRandomness, InMemoryDB>, HashMap<String, ProvingKeyMaterial>`) and validates responses as typed tagged `LedgerProofOutputTransaction`. |
| Indexer reads | `state_decode.rs` ported from `midnight-identity/develop@16bdc97...`, Apache-2.0. |

## Local validations

- `nix flake lock --update-input compact` → pinned `e53a8e88d32c561b69b2ed5f2e7e4c19b4c93d35`.
- `nix develop --command just codegen` → regenerated generated contract with constructor proof-data support.
- `nix develop --command cargo check -p midnight-did-runtime -p midnight-did-api -p midnight-did-uniffi` — pass.
- `nix develop --command cargo check -p midnight-did-runtime --features http` — pass.
- `nix develop --command cargo test -p midnight-did-runtime backend::tests -- --nocapture` — pass, 14 tests.
- `nix develop --command cargo test -p midnight-did-runtime --test ledger8_standalone -- --nocapture` — pass with explicit skip message because service env vars were absent.
- `nix develop --command cargo clippy --no-deps -p midnight-did-runtime -p midnight-did-api -p midnight-did-uniffi --all-targets -- -D warnings` — pass.

## Mobile target evidence

Installed user rustup targets include iOS and Android triples, but the Nix devshell Rust toolchain did not have `aarch64-apple-ios` std available. Command:

```text
cargo check -p midnight-did-runtime --target aarch64-apple-ios
```

Result: failed before crate compilation with `can't find crate for core`; no mobile pass is claimed.

## Standalone Ledger8 lifecycle

Added `crates/midnight-did-runtime/tests/ledger8_standalone.rs`. It is environment-gated by:

- `MIDNIGHT_DID_LEDGER8_STANDALONE=1`
- `MIDNIGHT_DID_LEDGER8_NODE_URL`
- `MIDNIGHT_DID_LEDGER8_INDEXER_URL`
- `MIDNIGHT_DID_LEDGER8_PROOF_URL`

In this run those variables were absent, so the harness printed a skip. No standalone node/indexer/proof-server lifecycle pass is claimed.

## Workspace boundary proof

The root workspace excludes the symlinked nested Ledger workspace with:

```toml
[workspace]
exclude = ["third_party/midnight-ledger"]
```

Root metadata evidence:

```text
midnight-ledger  8.2.0-rc.1  third_party/midnight-ledger/ledger/Cargo.toml
midnight-zswap   8.2.0-rc.1  third_party/midnight-ledger/zswap/Cargo.toml
```

`cargo tree -p midnight-did-runtime` shows Ledger crates from one physical source root: `third_party/midnight-ledger` (symlinked to the pinned Nix source for `b85f5d8e...`).
