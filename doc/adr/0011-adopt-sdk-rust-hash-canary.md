<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# ADR 0011 — Adopt the generic SDK-Rust hash surface as a downstream canary

**Status:** accepted · **Date:** 2026-09-17 · **Issue:** #81

## Context

`midnight-did-method` computes an off-chain holder-binding identifier as
`SHA-256(domain || NUL || payload)`. The domain vocabulary, normalization rules,
and holder-binding semantics are Midnight-specific and belong in this
repository. The SHA-256 operation itself is a generic primitive already owned
by Hyperledger Identus SDK-Rust.

The published SDK-Rust release train needs evidence from a real, independently
owned consumer. This repository must not transfer Midnight domain tags, DID
method behavior, Compact types, ledger/runtime types, or product policy into
the generic SDK merely to produce that evidence.

At the preimplementation baseline, `midnight-did-method` depended directly on
`sha2 0.10`. Its unique feature-tree output contained 204 lines, and a fresh
locked package check completed in 11.11 seconds on the development host. These
measurements are comparative observations, not performance guarantees.

SDK-Rust's accepted `0.1.x` minimum supported Rust version is 1.89. This
workspace declared 1.85 while its Nix gates already used a newer pinned nightly.
Adopting the SDK without changing the declaration would make the consumer's
compatibility metadata false: Cargo reports every crate in the pinned SDK train
as requiring Rust 1.89.

## Decision

Pin `identus-crypto` to the exact crates.io registry version
`=0.1.0-rc.1`. Disable default features and enable only `hash`. The lockfile
must resolve `identus-crypto`, `identus-core`, and `identus-derive` from the
crates.io registry at `0.1.0-rc.1` with checksums, not from SDK-Rust Git or
path sources.

Keep `domain_separated_sha256` and every Midnight-owned semantic in
`midnight-did-method`. The function constructs the bounded
`domain || NUL || payload` preimage locally and passes those bytes to the
generic one-shot `identus_crypto::hash::sha256` function. The SDK digest is
copied into the existing `[u8; 32]` return type, preserving this crate's public
contract.

The bounded preimage requires one allocation because the reviewed SDK surface
is intentionally one-shot. Adding a multipart API upstream is not justified by
this single canary: it would broaden the generic public contract before a
second use case demonstrates that cohesion. The allocation is explicit and
independently reversible.

Commit `Cargo.lock`. Retain no direct `sha2` dependency in
`midnight-did-method`; other workspace crates remain outside this slice.
Raise the workspace `rust-version` declaration from 1.85 to 1.89 so every
published crate states the dependency-imposed floor honestly. This is a
consumer compatibility decision, not an SDK specialization.

## Evidence required

- Existing TypeScript-derived golden vectors remain byte-for-byte identical.
- Independent differential vectors cover empty payloads, embedded NUL bytes,
  and domain/payload concatenation ambiguities.
- `cargo tree -e features` proves `identus-crypto` has only its `hash` feature
  and does not select curves, derivation, JWK, COSE, encodings, entropy, or
  compatibility features.
- `cargo metadata` proves `identus-crypto`, `identus-core`, and
  `identus-derive` resolve to exact `0.1.0-rc.1` crates.io packages with
  lockfile checksums.
- Native package tests, the repository's applicable WASM check, dependency,
  license, advisory, policy, and production-ready local gates pass.
- The PR records before/after dependency-cone and compile-duration evidence.

## Consequences

- The consumer proves a narrow real-world SDK seam without coupling SDK-Rust
  to Midnight.
- The exact registry pin is release-train canary evidence, not a runtime,
  device, FFI, certification, or production-support claim. Historical evidence
  from the prior Git-pinned canary remains comparative observation only.
- One small allocation replaces incremental hashing on this path. If profiling
  shows material impact or another consumer needs multipart input, propose a
  generic upstream API under a separate SDK issue and ADR.
- Consumers requiring Rust 1.85–1.88 cannot use this workspace revision. The
  repository's build compiler was already newer; the change aligns published
  metadata with the now-effective dependency floor.
- Rollback restores the direct `sha2` dependency and previous three-update
  implementation; no stored data, wire format, domain tag, or migration is
  involved.
