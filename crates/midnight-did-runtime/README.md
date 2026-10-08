<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-did-runtime

Codegen target and contract-transport layer for the Midnight DID Method
Rust port. This is the only crate in the workspace that touches
`midnight-compact-runtime` and the `midnight-ledger` crates directly.

## What lives here

- `contract/generated.rs` — **generated** by the Rust-codegen branch of
  the [Compact compiler](https://github.com/MediaNoxLabs/compact)
  (`compactc --rust`) from the vendored
  [`did.compact` 0.5.0](https://github.com/midnightntwrk/midnight-did)
  contract. Never hand-edit it; regenerate with `just codegen` and gate
  with `just codegen-check` (CI does).
- `contract_wrapper` — the concrete `Contract<B: Backend>` wrapper: one
  inherent method per exported circuit (create / rotate / recover /
  set/remove verification methods, relations, services, alsoKnownAs /
  deactivate), each encoding a typed call for its backend
  ([ADR 0008](../../doc/adr/0008-contract-abstraction-reform.md)).
- `contract_call` — `DidContractCall` enum + `DidLedgerSnapshot`: the
  typed wire representation of a contract invocation and the decoded
  ledger state.
- `backend` — the 3-method `Backend` trait and its implementations:
  - `RecordingBackend` — in-memory mock; every `submit_tx` is decoded
    back into a typed `DidContractCall` for assertion in tests.
  - `ResolverBackend` — serves a stored `DidLedgerSnapshot` for
    read-only resolution flows.
  - `LiveBackend` — provider-composed Ledger8 wallet + proof-server +
    node + indexer bridge used by the local standalone path; callers inject
    custody/proof/node/indexer providers and may reconcile a persisted
    finalized receipt after restart without resubmitting.
- `witnesses`, controller-key helpers — private-state plumbing the
  generated code requires.

## Position in the workspace

`midnight-did-api` builds operations *on top of* `Contract<B>`; this
crate owns the contract surface itself. Consumers that only need DID
parsing or document types should depend on `midnight-did-method` /
`midnight-did-domain` instead — this crate drags in halo2/arkworks
transitively and is not wasm-clean.

## Immutable Git consumption

External Cargo workspaces consume this crate from a full 40-character revision
of this repository:

```toml
[dependencies]
midnight-did-runtime = {
  git = "https://github.com/MediaNoxLabs/midnight-identity.git",
  rev = "<immutable-midnight-identity-revision>"
}
```

The runtime and Compact dependency pin every Ledger crate to
`MediaNoxLabs/midnight-ledger@2cce0f8f26e8ab1398af1c9ed61b087c28611a3f`.
Cargo does not inherit `[patch]` tables from Git dependencies, so the consumer
workspace must repeat the `midnight-proofs` patch from
[`scripts/ci/git-consumer-smoke.sh`](../../scripts/ci/git-consumer-smoke.sh),
pinning it to
`MediaNoxLabs/midnight-zk@532629b044a88473a7175f4a96c2511c91156136`.
The runtime itself pins Compact to the tree-equivalent merged revision
`MediaNoxLabs/compact@e17b2a42227efa84281c6ce41868b66eb909e236`.
Do not replace any member independently: source identity is part of the Rust
type identity, so a mixed registry/Git graph can produce incompatible Compact
and Ledger runtime types.

The supported external-consumer feature cone is native Rust only:

- the default feature set is empty;
- `http` enables the Rustls-backed proof/indexer HTTP providers;
- `node-subxt` enables Subxt node submission and its Tokio/futures support;
- `http,node-subxt` is the production-complete native cone exercised by the
  clean temporary-consumer check.

`midnight-did-uniffi` is not in this cone and its mock handle is not a
production API. This runtime cone is also not advertised as wasm-clean.

Git resolution is the portable default. Inside `nix develop .#rust`, local
builds may opt into the already materialised Nix sources without changing any
published manifest:

```console
scripts/cargo-with-nix-overrides.sh check -p midnight-did-runtime --features http,node-subxt
```

The wrapper applies ephemeral path patches for the exact Ledger and Compact
sources. Ordinary `cargo` commands continue to prove clean Git consumption.
