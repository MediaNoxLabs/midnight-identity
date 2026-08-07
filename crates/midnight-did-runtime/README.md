<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-did-runtime

Codegen target and contract-transport layer for the Midnight DID Method
Rust port. This is the only crate in the workspace that touches
`compact-runtime` and the `midnight-ledger` crates directly.

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
  - `LiveBackend` — the wallet + proof-server + indexer bridge;
    `todo!()` in v0.5.0, tracked as the LiveBackend backlog issue.
- `witnesses`, controller-key helpers — private-state plumbing the
  generated code requires.

## Position in the workspace

`midnight-did-api` builds operations *on top of* `Contract<B>`; this
crate owns the contract surface itself. Consumers that only need DID
parsing or document types should depend on `midnight-did-method` /
`midnight-did-domain` instead — this crate drags in halo2/arkworks
transitively and is not wasm-clean.
