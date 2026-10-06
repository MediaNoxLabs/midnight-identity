# Issue #9 LiveBackend evidence

SPDX-License-Identifier: Apache-2.0

## Implemented shape in this worktree

`midnight-did-runtime::LiveBackend` is dependency-injected. Mutating calls run:

1. decode the in-process `DidContractCall` command;
2. read live indexer state through `LedgerIndexerProvider`;
3. execute generated Rust `did.compact` through `GeneratedDidExecutor`;
4. extract Compact proof data into `DidPrePartitionContractCall`;
5. hand generated proof material to the wallet provider;
6. balance, prove via `/prove-tx`, finalize Ledger transaction state, submit through a node envelope/finality provider, wait for finality, and reconcile.

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


## Hosted CI follow-up: passport artifact manifest drift

Hosted VC codegen drift initially failed after the merge because `artifacts/passport-vault-ledger8/manifest.json` still contained the pre-merge repository Compact input and lock digests. The artifact manifest was regenerated with:

```bash
nix develop .#factory --command node scripts/compact/passport-vault-ledger8-artifacts.mjs --out artifacts/passport-vault-ledger8/manifest.json
```

The focused artifact test command passed locally with 6 passed and 3 checkout-mode tests skipped because `OXID_REFERENCE_ROOT` was not configured:

```bash
nix develop .#factory --command node --test tests/compact/passport-vault-ledger8-artifacts.test.mjs
```

## Acceptance follow-up at head after `b447e5d`

Independent review correctly identified that the previous `ledger8_standalone` test was only an environment assertion. It has been replaced with an explicit service protocol probe that does not claim lifecycle success.

Local service probe against the supplied stack:

```bash
MIDNIGHT_DID_LEDGER8_STANDALONE=1 \
MIDNIGHT_DID_LEDGER8_NODE_URL=http://127.0.0.1:9944 \
MIDNIGHT_DID_LEDGER8_INDEXER_URL=http://127.0.0.1:8088 \
MIDNIGHT_DID_LEDGER8_PROOF_URL=http://127.0.0.1:6300 \
nix develop --command cargo test -p midnight-did-runtime --test ledger8_standalone -- --nocapture
```

Result: failed intentionally before any deploy/create claim. Evidence:

- node `system_health`: healthy / not syncing;
- node `midnight_ledgerVersion`: `=8.0.2`;
- node RPC methods include `author_submitExtrinsic`, `chain_getFinalizedHead`, `chain_getBlock`, and `midnight_contractState`;
- indexer GraphQL schema includes `contractAction` at `/api/v3/graphql`;
- proof server `/health` returns HTTP 200;
- no wallet/funding/balancing/signing API is exposed by the node/indexer/proof-server stack (`wallet_like` RPC method set was empty), and no `MIDNIGHT_DID_LEDGER8_WALLET_PROVIDER_URL` was supplied.

Therefore a pure-Rust end-to-end deploy/create lifecycle is still blocked by the absence of an external wallet/funding/balancing/signing provider, not by node/indexer/proof health. No standalone lifecycle pass is claimed. Focused follow-up: #102.

Custody follow-up:

- `DidPrivateTranscriptOutputs` is no longer public/exported or cloneable.
- `DidConstructorProofMaterial`, `DidPrePartitionContractCall`, and `DidDeploymentRequest` no longer derive `Clone`.
- Secret witness transcript outputs are private fields and are consumed only at the Ledger construction boundary.
- Debug output omits witness transcript fields; no serde derives are present on the proof DTOs.
- `HttpNodeProvider` now validates `author_submitExtrinsic` submissions by scanning finalized blocks for the exact submitted extrinsic bytes. Provider-specific custom submit methods must return a finality receipt object and own that finality contract.

Focused checks:

- `nix develop --command cargo test -p midnight-did-runtime backend::tests -- --nocapture` → 25 passed.
- `nix develop --command cargo test -p midnight-did-runtime --features http backend::tests::http_node_provider_waits -- --nocapture` → passed.
- `nix develop --command cargo check -p midnight-did-runtime --features http` → passed.

## Local Rust Ledger8 wallet provider addendum

Added `LocalLedger8WalletProvider` in `crates/midnight-did-runtime/src/backend.rs`.
It uses the same `b85f5d8e...` Ledger8 graph and does not depend on Oxid or any
Ledger9/10/D941 revision. The provider:

- consumes a one-shot `LocalLedger8DustSeed` and zeroizes it after deriving the Ledger DUST key;
- loads authoritative DUST state through `LocalLedger8DustStateProvider`, so production code resumes from a private checkpoint/indexer replay rather than a public caller-fabricated genesis list;
- retains only the derived Ledger DUST secret key and synchronized mutable `DustLocalState<DefaultDB>` internally;
- constructs typed Ledger8 deploy transactions containing `ContractDeploy<DefaultDB>`;
- constructs typed Ledger8 call transactions from generated `PrePartitionContractCall<DefaultDB>`;
- balances DUST fees locally with bounded iterations;
- emits the exact tagged Ledger8 `/prove-tx` request shape;
- validates custody boundaries by omitting DUST seed/witness material from `Debug` and public DTOs;
- returns a typed `FinalizedDidTransaction` state that distinguishes tagged Ledger8 transaction bytes from an encoded outer Substrate extrinsic;
- prevents `HttpNodeProvider(author_submitExtrinsic)` from submitting raw tagged Ledger8 transaction bytes as if they were an outer extrinsic.

Focused local evidence after this addendum:

- `node scripts/ci/target-plan.mjs --base origin/develop --head HEAD` → `production-ready/full: policy, rust, unit, wasm, coverage, did-codegen, vc-codegen`.
- `rustfmt --edition 2024 crates/midnight-did-runtime/src/backend.rs crates/midnight-did-runtime/src/lib.rs` → passed. (`cargo fmt --all` still attempts to rewrite read-only Nix-store Ledger/Compact sources and fails with permission errors outside this repository.)
- `cargo check -p midnight-did-runtime --features http` → passed.
- `cargo test -p midnight-did-runtime backend::tests -- --nocapture` → 26 passed.

`crates/midnight-did-runtime/tests/ledger8_standalone.rs` remains honest: full
standalone submit/finality evidence still requires a node adapter that wraps the
proven Ledger8 transaction in the chain's accepted outer Substrate extrinsic or a
provider-owned finality receipt. The local wallet provider narrows #102 by
providing wallet construction/balancing/prove-request primitives with checkpoint
injection, but #102 is not closed and no finalized deploy/create lifecycle is
claimed until standalone synchronization/submission/finality is exercised against
node/indexer/proof.

TS parity note: the Rust provider covers the same DID wallet boundary exercised by
`third_party/midnight-did/packages/api/src/wallet-provider.ts` at the pinned DID
reference `42a8e4aca1c6043f7f2b73463fa3aa3cd3d48b06`: deployment/call material
is accepted from the generated contract API, wallet code owns transaction
construction and balancing, proof transport is provider-injected, and the final
submission boundary remains separate from contract execution. No TypeScript or
JS runtime bridge is introduced.
