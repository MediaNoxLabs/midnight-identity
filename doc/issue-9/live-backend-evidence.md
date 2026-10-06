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

## PR #101 merge-reconciliation addendum

Resolved the dirty merge against `origin/develop` at `16bdc97` while preserving:

- Ledger input `MediaNoxLabs/midnight-ledger@b85f5d8e503fd1d7a1b128bbc1d7156baf823a65`.
- Compact input `MediaNoxLabs/compact@e53a8e88d32c561b69b2ed5f2e7e4c19b4c93d35`.
- Draft PR status.

Additional mandatory review fixes:

- `DidDeploymentRequest` is now a public deployment-material DTO without the constructor private state type parameter or `initial_private_state` field.
- Constructor execution persists post-constructor private state internally through the injected private-state store.
- Deployment material builds an actual Ledger8 `midnight_ledger::structure::ContractDeploy<DefaultDB>` with `ContractState { data, operations, maintenance_authority, balance }` and nonce.
- Mutating calls still convert directly into Ledger8 `midnight_ledger::construct::PrePartitionContractCall<DefaultDB>`.
- Generated `.zkir` assets now carry local provenance in `crates/midnight-did-runtime/assets/keys/README.md`.

Merge-reconciliation validations:

- `nix develop --command just codegen-check` → passed.
- `nix develop --command just codegen-vc-check` → passed after regenerating develop VC bindings for Compact runtime `0.16.102`.
- `nix develop --command node scripts/ci/target-plan.mjs` → `production-ready/full`.
- `nix develop --command cargo metadata --format-version 1` → `midnight-ledger 8.2.0-rc.1` and `midnight-zswap 8.2.0-rc.1`, both from `third_party/midnight-ledger`.
- `nix develop --command cargo check -p midnight-did-runtime -p midnight-did-api -p midnight-did-uniffi` → passed.
- `nix develop --command cargo check -p midnight-did-runtime --features http` → passed.
- `nix develop --command cargo test -p midnight-did-runtime backend::tests -- --nocapture` → 24 passed.
- `nix develop --command cargo test -p midnight-did-runtime --test ledger8_standalone -- --nocapture` → gated harness skip path passed; standalone services still not configured locally.
- `nix develop --command cargo test -p midnight-did-api -p midnight-did-uniffi -- --nocapture` → passed.
- `nix develop --command cargo clippy --no-deps -p midnight-did-runtime -p midnight-did-api -p midnight-did-uniffi --all-targets -- -D warnings` → passed.
- `nix develop --command cargo check --workspace` → passed after the develop VC generated bindings were refreshed for Compact runtime `0.16.102`.

## Final merge-gate evidence before PR #101 push

Additional production-ready checks after resolving the staged merge:

- `nix develop --command node scripts/ci/target-plan.mjs --base origin/develop --head HEAD` → `production-ready/full: policy, rust, unit, wasm, coverage, did-codegen, vc-codegen`.
- `nix develop --command node scripts/factory/check.mjs` → 48/48 pass; factory audit 0 local findings.
- `nix develop --command npx dev-loops@1.0.2 doctor` → 5/7 pass; warnings only for unavailable `subagent` command and stale-but-explicit pinned `dev-loops@1.0.2` versus latest prerelease.
- `nix develop --command npx dev-loops@1.0.2 gates` → listed configured draft/pre-approval gate surfaces; no repository mutation.
- `nix develop --command just fmt-check` → passed.
- `nix develop --command just lint` → passed.
- `nix develop --command just build` → passed.
- `nix develop --command just test` → 692/692 workspace tests passed plus 10/10 `midnight-vc-families --all-features` tests passed.
- `nix develop --command just coverage-gate` → passed, total line coverage 88.75% (floor 87%).
- `nix develop --command just coverage-ratchet` → measured 88.75%, floor 87%, headroom 1.75.
- `nix develop --command cargo check -p midnight-did-runtime --target aarch64-apple-ios` → unavailable before crate compilation: target std missing (`can't find crate for core`); no mobile pass claimed.
- `git diff --check --cached` → passed.
- Forbidden revision/package grep for retired prototype revisions and retired Ledger major labels → no matches in the staged tree.

Local review (distinct from tests) checked the staged diff for the mandatory review findings:

- `DidDeploymentRequest` exposes generated non-secret deployment state only (`initial_contract_state`, `initial_zswap_local_state`, constructor transcript/proof material) and has no `Debug`, serde, `PS`, or `initial_private_state` public surface.
- `GeneratedDidExecutor::deployment_request` stores the constructor's `current_private_state` through the injected private-state store before returning deployment material.
- The wallet deployment port receives the full `DidDeploymentRequest`; wallet/custody supplies the typed Ledger8 `LedgerDeploymentConfig` operation map, maintenance authority, and nonce.
- `DidDeploymentRequest::to_contract_deploy` constructs Ledger8 `midnight_ledger::structure::ContractDeploy<DefaultDB>` rather than encoding the constructor as a normal call.
