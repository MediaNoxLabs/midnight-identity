<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-did-method

Midnight method profile for the Rust port of the [Midnight DID Method][adr].

[adr]: https://github.com/MediaNoxLabs/midnight-identity/blob/main/doc/adr/0003-crate-split-2-to-4-with-umbrella.md

This crate sits between [`midnight-did-domain`][domain] (pure W3C DID Core
types) and [`midnight-did-api`][api] (the async operation layer). It hosts
the pieces that are **specific to the Midnight method** but do not need
the on-chain runtime or the operation-layer abstractions.

[domain]: ../midnight-did-domain
[api]: ../midnight-did-api

## What lives here

- `midnight_did` — `did:midnight:<network>:<id>` string types, parsing,
  and subject-id helpers (moved from `midnight-did-domain::midnight`).
- `network_mapping` — runtime ↔ domain network identifier mapping
  (moved from `midnight-did-api::network_mapping`).

## Consumer parser convergence

The crate exposes the lightweight parser surface needed by current consumers:

- `parse_midnight_did_parts` validates a strict four-segment
  `did:midnight:<network>:<hex64>` identifier, rejects oversized input and
  whitespace/control characters, accepts mixed-case on-chain hex while
  preserving lowercase off-chain canonical form, and returns typed errors.
- `parse_midnight_key_id` splits a DID-URL key id on the last `#`, validates
  the bare DID, and retains the returned fragment with its leading `#`.
- Long-form off-chain DIDs keep the Oxid rules: lowercase-only off-chain hash
  hex plus unpadded base64url state text using only `[A-Za-z0-9_-]` and a
  legal base64url length.

Two fragment-to-method-id conventions intentionally remain separate:

1. The portal DID-manager convention pads the retained `#fragment` text to a
   32-byte method id for on-chain verification-method references.
2. Holder-binding uses
   `SHA-256("midnight:offchain:holder-method-id:v1" || NUL || #fragment)` via
   `holder_binding::hash_offchain_did_method_id`.

Do not combine these algorithms or use one convention to verify data produced
for the other.

After consumer cutovers pin an immutable shared revision, the following
consumer helpers should become deletable in their own repositories:

- `input-output-hk/lace-id-portal`: `parse_midnight_did`, `parse_key_id`, and
  their private `is_hex64` helper in `crates/did-midnight/src/lib.rs`.
- `MediaNoxLabs/oxid`: the `MidnightDid`, `MidnightDidError`, and
  `MidnightNetwork` parser-only duplication in
  `crates/identity/domain/src/lib.rs`, where callers only need DID parsing
  rather than the broader identity-domain types.

## When to depend on this crate vs. the others

- **Resolver-only** consumers (read a `did:midnight:*` from the ledger,
  return a DID Document JSON): depend on
  `midnight-did-domain` + `midnight-did-method` + a public-data provider.
  Skip the api crate entirely.
- **Write-side** consumers (mutate a DID — create / update / deactivate):
  depend on `midnight-did-api`, which transitively pulls this crate in.
- **Mobile / monolithic** consumers can pull the
  [`midnight-did`](../midnight-did) umbrella crate and let it re-export
  everything.

See [ADR 0003][adr] for the dependency-layering rationale.
