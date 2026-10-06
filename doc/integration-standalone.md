<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Live integration: resolving a real DID through IndexerBackend

The issue-#4 acceptance procedure — proven end-to-end on 2026-08-07:
deploy a `did:midnight` with the TS reference stack, then resolve it
from pure Rust via `midnight-did-indexer` and diff against the TS
reference resolver.

## 1. Start the standalone stack (fixed ports)

```bash
docker compose -p did-int -f infra/standalone.yml up -d --wait
```

Node `:9944`, indexer `:8088` (GraphQL at `/api/v3/graphql`),
proof-server `:6300` — exactly the TS `StandaloneConfig` endpoints.

## 2. Deploy a DID (TS reference tooling)

Clone `midnightntwrk/midnight-did` at the pinned contract commit
(42a8e4a / 0.5.0), `pnpm install --frozen-lockfile`, build
`...-contract...` and `...-api...` (needs the `compact` CLI + node 24),
then run a deploy script against `StandaloneConfig` with the genesis
seed `0x…01`: `buildWalletAndWaitForFunds` →
`registerForDustGeneration` → `configureProviders` → `initPrivateState`
→ `createDID` (+ optional `addVerificationMethod` /
`addVerificationMethodRelation` for a richer document). ~1 min
including proving.

⚠️ The contract version matters: a pre-0.5.0 workspace deploys an
18-field ledger (`controllerPublicKey: Bytes<32>`, no recovery
authority) whose chunk-0 layout the 0.5.0 decoder rejects — by design
(`missing ledger slot` errors instead of mis-decoding).

## 3. Resolve from Rust

```bash
cargo run -p midnight-did-indexer --example live_resolve -- <contract-address-hex>
```

(Optional args: indexer GraphQL URL, network.) Prints
`{didDocument, didDocumentMetadata}` JSON via the full read path:
indexer GraphQL → `ContractState` deserialize →
`state_decode::decode_ledger_snapshot` → `midnight-did-api::resolve`.

## 4. Parity result (2026-08-07)

Against the upstream `MidnightDIDResolver` (same ledgerReader the
resolver service uses) on the same DID:

- `didDocumentMetadata`: **byte-identical** (`created`, `updated`,
  `versionId`).
- `didDocument`: identical except one known cosmetic deviation — the
  TS document type serializes **empty** `alsoKnownAs`/`service` as
  explicit `null`, we omit the fields (semantically equivalent;
  tracked as a parity-polish issue).
