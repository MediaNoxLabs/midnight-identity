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

## What's here

| Crate | Purpose | Publishable |
|---|---|---|
| `midnight-did-domain` | Pure-data W3C DID Core model + crypto codecs (zero `midnight-*` deps, wasm-clean) | yes |
| `midnight-did-method` | `did:midnight:*` parsing, network mapping, MOD1 offchain codec | blocked¹ |
| `midnight-did-api` | Async operation builders (create / update / rotate / recover / resolve / deactivate) over `Contract<B: Backend>` | blocked¹ |
| `midnight-did-runtime` | `compactc --rust` codegen target: generated contract bindings, `Backend` trait, mock + resolver backends | blocked¹ |
| `midnight-did` | Umbrella re-export crate | blocked¹ |
| `midnight-did-uniffi` | Swift / Kotlin / Python bindings (UniFFI) | no (by design) |
| `midnight-did-cli` | Reference CLI demo | no (by design) |

¹ crates.io publication is blocked until the upstream `midnight-ledger`
crates and `compact-runtime` are published; consume via git until then.
See the publishing issue in the backlog.

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
use compact_runtime::ContractAddress;

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
just --list            # available recipes
just ci                # fmt-check + lint + build + test + coverage-gate
just codegen-check     # regen generated.rs, assert no drift
just coverage          # HTML coverage report (line floor: see justfile)
```

An optional [pi.dev operator shell](./doc/pi-development.md) layers the
`dev-loops` workflow on top of the devshell.

**Branch model:** `rust-codegen` is the stable mainline; `develop` is
the integration branch — PRs target `develop`.

## CI

Every PR runs: fmt + clippy (`-D warnings`), tests on Linux + macOS,
a `wasm32-unknown-unknown` build of the wasm-clean `midnight-did-domain`
crate, a line-coverage floor (cargo-llvm-cov), and the codegen
drift-check against the flake-pinned compactc.

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
  (`codegen-rust` branch) and `compact-runtime`.
- [midnightntwrk/midnight-did](https://github.com/midnightntwrk/midnight-did)
  — TypeScript reference implementation + the `did.compact` contract
  (vendored here as a submodule).
- [midnightntwrk/midnight-did-resolver](https://github.com/midnightntwrk/midnight-did-resolver)
  — TS resolver service; the model for the planned Rust resolver.
- [midnightntwrk/midnight-verifiable-credentials](https://github.com/midnightntwrk/midnight-verifiable-credentials)
  — Compact-first VC stack; the model for the planned shared SSI crates.
