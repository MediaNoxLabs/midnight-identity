<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-did-resolver in Rust — design proposal

**Date:** 2026-08-07 · **Status:** research → proposal
**Source repo analyzed:** [midnightntwrk/midnight-did-resolver](https://github.com/midnightntwrk/midnight-did-resolver)
(Fastify 5 monorepo: resolver service + DID manager service +
secret-storage + VitePress docs; pinned to the same midnight-did
**0.5.0** cohort this workspace ports)

## 1. What the TS resolver is

- **Standalone HTTP service** (not a DIF universal-resolver driver).
  Routes: `GET /resolve/:did`, `POST /resolve` (body
  `{did, indexerUrl?, indexerWsUrl?}`), `GET /health`, `GET /ready`,
  `GET /docs` (Swagger), `GET /` (demo UI).
- **W3C DID Resolution envelope**: `didDocument` /
  `didDocumentMetadata` (`created`, `updated`, `versionId`,
  `deactivated`) / `didResolutionMetadata` (`contentType`, `error`).
  Error codes: `notFound` (404) | `invalidDid` (400) |
  `networkMismatch` (400) | `internalError` (500).
- **Pipeline**: parse `did:midnight:*` → reject offchain-network DIDs +
  network mismatches → `queryContractState(contractAddress)` via the
  **GraphQL indexer** (`indexer-public-data-provider`) →
  `ContractState.deserialize` → generated `ledger()` accessor →
  `LedgerToDomain.ledgerStateToDIDDocument` + metadata.
- **Hardening worth copying**: per-request indexer override guarded by
  an SSRF policy (rejects loopback/private/link-local hosts and embedded
  credentials); resolver-instance LRU keyed by
  `endpoint|ws|network` (no result caching — resolution is always
  live); 15 s resolve timeout; 64 KB body limit; security headers.
- Config via env: `RESOLVER_HOST/PORT`, `MIDNIGHT_INDEXER_HTTP_URL`
  (`.../api/v3/graphql`), `MIDNIGHT_INDEXER_WS_URL`,
  `MIDNIGHT_NETWORK` (optional expected-network guard). Docker image on
  ghcr with SBOM/provenance.

## 2. What our crates already cover

| TS piece | Rust equivalent | Status |
|---|---|---|
| `parseMidnightDIDString` / `parseMidnightDID` | `midnight-did-method::midnight_did` | done |
| DID document + metadata types | `midnight-did-domain::did_document` | done |
| Ledger → document projection | domain/api mapping layer | done |
| Resolve orchestration | `midnight-did-api::resolve` over `Contract<B: Backend>` | done (mock/stored-snapshot backends) |
| Error taxonomy | typed errors in api/domain | done — map to the 4 resolution codes |

The HTTP layer itself is thin: an axum service with the routes above is
roughly 500 lines.

## 3. The one real gap — and it's the LiveBackend gap

The TS service's entire chain access is one call:
`publicDataProvider.queryContractState(address)` + state
deserialization into the generated ledger accessor. The Rust
equivalent needs:

1. an **indexer GraphQL client** (the `contractState` query, HTTP;
   WS subscription optional for parity),
2. **contract-state deserialization** into the types
   `midnight-did-runtime`'s generated bindings read
   (`midnight-ledger`'s `ContractState::deserialize` is the analogue —
   the crates exist in `third_party/midnight-ledger`),
3. wiring both as a **read-only `Backend`** (`read_snapshot` real,
   `submit_tx` unsupported).

This is exactly "Approach A — indexer-backed read-only LiveBackend"
already brainstormed for the LiveBackend track. **The resolver app is
the motivating consumer for that work**: shipping the read path gives
both a live resolver and the first production `Backend`.

## 4. Proposed shape

New crate `midnight-did-resolver` (bin + lib) in this workspace:

- axum + tokio; routes/envelope/status mapping per §1 (typed errors —
  not the TS string-matching classifier).
- Backend: `IndexerBackend` (read-only `Backend` impl) from the
  LiveBackend track; `ResolverBackend` (stored snapshot) retained for
  tests and offline fixtures.
- SSRF policy + optional `MIDNIGHT_NETWORK` guard ported as-is.
- Container image + compose file against the standalone stack
  (node + indexer + proof server) we already run for TS-parity tests.

**Conformance target:** match the TS service's envelope byte-for-byte
where deterministic, and validate against the same 27-test lifecycle
suite (resolve legs) on the standalone stack.

## 5. Sequencing

1. Indexer GraphQL client + state deserialization (LiveBackend
   read-only path) — prerequisite.
2. `midnight-did-resolver` crate: routes + envelope + config + SSRF
   policy, `ResolverBackend`-backed tests.
3. Integration: standalone stack, resolve a DID created by the TS
   suite; envelope diff against the TS resolver's output.
4. Optional parity extras: Swagger, WS indexer support, demo UI.
