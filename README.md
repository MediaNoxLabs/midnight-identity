<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-identity

[![CI](https://github.com/MediaNoxLabs/midnight-identity/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/MediaNoxLabs/midnight-identity/actions/workflows/ci.yml)
[![version](https://img.shields.io/badge/version-v0.5.0-blue)](./CHANGELOG.md)
[![license](https://img.shields.io/badge/license-Apache--2.0-green)](./LICENSE)

**Rust libraries for the Midnight SSI domain.**

This repository is the home of the self-sovereign-identity stack for
the [Midnight](https://midnight.network) blockchain, implemented in
Rust. Today that means a complete, byte-parity port of the
[`did:midnight` DID method](https://github.com/midnightntwrk/midnight-did)
(W3C DID Core data model, method profile, async operation API, Compact
contract bindings, FFI). The roadmap extends the same foundations to
Verifiable Credentials and a standalone DID resolver service — tracked
in the [issue backlog](https://github.com/MediaNoxLabs/midnight-identity/issues).

**The Verifiable Credentials track has started** (issue #13): the
credential model and the generated bindings for the Midnight VC
Compact contracts now ship as `midnight-vc-domain`, `midnight-vc-runtime`,
and the opt-in `midnight-vc-families` bindings.

## What's here

| Crate | Purpose | Publishable |
|---|---|---|
| `midnight-did-domain` | Pure-data W3C DID Core model + crypto codecs (zero `midnight-*` deps, wasm-clean) | yes |
| `midnight-did-method` | Runtime-independent `did:midnight:*` parsing, network mapping, holder binding, and MOD1 offchain codec | yes |
| `midnight-did-api` | Async operation builders (create / update / rotate / recover / resolve / deactivate) over `Contract<B: Backend>` | blocked¹ |
| `midnight-did-runtime` | `compactc --rust` codegen target: generated contract bindings, `Backend` trait, mock + resolver backends | blocked¹ |
| `midnight-did` | Umbrella re-export crate | blocked¹ |
| `midnight-did-indexer` | Indexer GraphQL client + read-only `IndexerBackend` (live resolution) | blocked¹ |
| `midnight-did-resolver` | HTTP DID resolution service (axum) — Rust counterpart of `midnight-did-resolver` (TS) | no (service binary) |
| `midnight-did-uniffi` | Swift / Kotlin / Python bindings (UniFFI) | no (by design) |
| `midnight-did-cli` | Reference CLI demo | no (by design) |
| `midnight-vc-domain` | Pure-data VC credential model: schema/claim descriptors, composition manifests, status vocabulary (zero `midnight-*` deps, wasm-clean) | yes |
| `midnight-vc-runtime` | `compactc --rust` codegen target for the VC contracts (credentials / iso-registry / same-holder bindings) | no (`publish = false`)² |
| `midnight-vc-families` | Feature-gated generated bindings for credential families (`digital-passport` today) | no (`publish = false`)² |
| `midnight-passport-account-source` | Authenticated Passport account-custody Compact source and Ledger 9.1 compatibility descriptor (no runtime dependencies) | yes |
| `midnight-passport-vault-source` | Authenticated Passport Vault Compact source, provenance, and circuit baselines (no runtime dependencies) | yes |

¹ crates.io publication is blocked until the upstream `midnight-ledger`
crates and `midnight-compact-runtime` are published; consume via git until then.
See the publishing issue in the backlog.

² same upstream blocker, and it is a generated-code artifact rather
than a library surface — see [doc/publishing.md](./doc/publishing.md).

## Architecture

```
                ┌───────────────────────────────┐
                │     midnight-did-domain       │   pure-data W3C DID Core
                │   (no midnight-* deps)        │   + crypto codecs
                └──────────────┬────────────────┘
                               │
                               ▼
                ┌───────────────────────────────┐
                │     midnight-did-method       │   did:midnight:* parsing
                │                               │   + MOD1 offchain codec
                └──────────────┬────────────────┘
                               │
                               ▼
                ┌───────────────────────────────┐
                │      midnight-did-api         │   operation builders
                │                               │   + ledger mappers
                └──────────────┬────────────────┘
                               │
                               ▼
                ┌───────────────────────────────┐
                │    midnight-did-runtime       │   compactc --rust output
                │   Contract<B> + Backend trait │   + DidContractCall enum
                └──────────────┬────────────────┘
                               │
                               ▼
                ┌───────────────────────────────┐
                │        midnight-did           │   umbrella re-export
                │                               │   (monolithic consumers)
                └───────────────────────────────┘
```

The dep direction is strict: domain ← method ← api ← runtime ← umbrella.
A resolver stops at `midnight-did-method`; a wallet pulls the umbrella.
Full breakdown in [`doc/architecture.md`](./doc/architecture.md); design
history in [`doc/adr/`](./doc/adr/) (ADRs 0001–0008).

The contract bindings in `midnight-did-runtime/src/contract/` are
**generated** — by the Rust-codegen branch of the
[Compact compiler](https://github.com/MediaNoxLabs/compact) (flake input
`github:MediaNoxLabs/compact/codegen-rust`) from the vendored
[`did.compact` 0.5.0](https://github.com/midnightntwrk/midnight-did)
contract. Never hand-edit them; run `just codegen`.

## Quick start

In a test, drive the contract through `RecordingBackend` — every
`submit_tx` is decoded back into a typed `DidContractCall` you can
assert on without spinning up a halo2 prover:

```rust
use midnight_did_runtime::{
    Contract,
    backend::RecordingBackend,
    contract_call::DidLedgerSnapshot,
};
use midnight_did_method::midnight_did::ContractAddress;

let snapshot = DidLedgerSnapshot::default(); // or a real fixture
let addr = ContractAddress::default();
let network = midnight_did_method::Network::Undeployed;

let contract = Contract::new(
    RecordingBackend::with_snapshot(snapshot),
    addr,
    network,
);

// drive any of the inherent `Contract<B>` methods, or hand `&contract`
// to the operation builders in midnight-did-api…

let recorded = contract.backend().recorded_calls();
assert_eq!(recorded.len(), 1);
```

In production code the same surface accepts a `LiveBackend`
(`submit_tx`/`read_snapshot` are `todo!()` in v0.5.0 — the
wallet+proof-server+indexer bridge is the tracked follow-up).

## Development

Everything runs inside the nix devshell:

```bash
nix develop            # toolchain + compactc + third_party mounts
nix develop .#rust     # light Rust gates; no compactc/proving closure
just --list            # available recipes
just ci                # fmt-check + lint + build + test + coverage-gate
just codegen-check     # regen generated.rs, assert no drift
just codegen-vc-check  # same, for the VC contract bindings
just coverage          # HTML coverage report (line floor: see justfile)
```

An optional [pi.dev operator shell](./doc/pi-development.md) layers the
`dev-loops` workflow on top of the devshell.

Repository delivery follows the [AI Software Factory](./doc/factory/README.md):
issue-backed worktrees, `prototype` or `production-ready` profiles,
path-proportional validation, draft-first pull requests, exact-head metrics,
and fail-closed worktree cleanup. The repository is the reusable Midnight
library boundary; application policy and deployment remain in consumers.

**Branch model:** `rust-codegen` is the stable mainline; `develop` is
the integration branch — PRs target `develop`.

## CI

Every PR runs the change planner plus the policy/factory lane. Affected Rust
crates add focused format, clippy, compile, test, coverage, and WASM lanes in
the light `.#rust` shell. DID or VC Compact inputs and generated artifacts add
their corresponding codegen drift lane in the full shell against the
flake-pinned `compactc`. Documentation-only and factory/CI-only changes do not
construct the Rust or Compact closure. Unknown/build-system changes, promotion
branches, scheduled audits, and manually requested full runs fail closed to
the complete public matrix. All jobs use public binary caches; light jobs also
use their explicit Cargo source cache. No CI or release gate requires a
FlakeHub account.

## Community

- [CONTRIBUTING.md](./CONTRIBUTING.md) — workflow, DCO + GPG signing,
  validation tiers.
- [SECURITY.md](./SECURITY.md) — how to report vulnerabilities
  (never via public issues).
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
- [Issue backlog](https://github.com/MediaNoxLabs/midnight-identity/issues)
  — the primary planning surface for this repo.

## Related repositories

- [MediaNoxLabs/compact](https://github.com/MediaNoxLabs/compact) —
  Compact compiler fork carrying the `--rust` codegen backend
  (pinned compiler revision) and `midnight-compact-runtime`.
- [midnightntwrk/midnight-did](https://github.com/midnightntwrk/midnight-did)
  — TypeScript reference implementation + the `did.compact` contract
  (vendored here as a submodule).
- [midnightntwrk/midnight-did-resolver](https://github.com/midnightntwrk/midnight-did-resolver)
  — TS resolver service; the model for the planned Rust resolver.
- [midnightntwrk/midnight-verifiable-credentials](https://github.com/midnightntwrk/midnight-verifiable-credentials)
  — Compact-first VC stack; the model for the planned shared SSI crates.
