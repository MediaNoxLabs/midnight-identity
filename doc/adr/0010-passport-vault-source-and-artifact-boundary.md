<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# ADR 0010 — Separate Passport Vault source from ledger-specific artifacts

**Status:** accepted · **Date:** 2026-09-15 · **Issue:** #47

## Context

Oxid temporarily distributes the reviewed Passport Vault Compact source because
the original example repository is private. The contract is reusable identity
infrastructure, but its generated Rust, Midnight Ledger runtime, ZKIR compiler,
proving parameters, and keys create a much heavier dependency and build graph
than the source itself. Moving that complete graph into every consumer would
change repository ownership without improving build time or reproducibility.

ADR 0009 permits a crate split when it isolates dependency weight, establishes
an independent release boundary, or measurably reduces compilation blast
radius. Passport Vault satisfies all three reasons.

## Decision

`midnight-passport-vault-source` is the authoritative lightweight distribution
of the reviewed Ledger 8 contract source. It contains the byte-identical source,
Apache-2.0 sidecar, provenance, compatibility metadata, and reviewed circuit
baselines. It has no runtime dependencies and is publishable independently.

Generated bindings and proving artifacts will live in a separate
ledger-specific leaf package or pinned Nix derivation. That leaf owns Compact
code generation, source-closure construction, ZKIR compilation, parameters,
artifact digests, and ABI/state-vector drift checks. Proving keys remain
reproducibly derived rather than committed.

Consumers pin a release or full Git revision. They must not read a sibling
checkout or moving branch. Oxid keeps its product ports, custody, policy,
submission lifecycle, and UI; its existing Passport Vault adapter becomes the
boundary that consumes the upstream descriptor and artifacts.

The Ledger 9 port is a separate compatibility package or major version. Cargo
features will not select mutually exclusive ledger graphs.

## Migration sequence

1. Publish the authenticated source package and descriptor upstream.
2. Move deterministic Compact/ZKIR artifact generation upstream and record
   cold/warm generation and consumer-build measurements.
3. Pin the green immutable upstream output in Oxid and prove source, ABI, state,
   circuit, and artifact parity at the exact consumer head.
4. Remove Oxid's duplicate source only after that cutover is green; retain the
   previous pin as the rollback point and supersede Oxid ADR-0053.

## Consequences

- Source consumers avoid the Midnight Ledger, Halo2, Compact runtime, and ZKIR
  dependency graph.
- Expensive artifacts become cacheable once per immutable upstream input rather
  than regenerated in every product checkout.
- Contract policy changes remain explicit source-version reviews.
- This first slice establishes ownership but does not yet authorize Oxid to
  delete its local source.

