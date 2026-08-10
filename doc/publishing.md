<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Crate publishing policy

## Publishable set

| Crate | crates.io status | Blocker |
|---|---|---|
| `midnight-did-domain` | **publishable now** | none — zero `midnight-*` deps |
| `midnight-did-method` | blocked | `midnight-base-crypto`, `compact-runtime` unpublished |
| `midnight-did-runtime` | blocked | `midnight-ledger` crates, `compact-runtime` unpublished |
| `midnight-did-api` | blocked | transitively via method + runtime |
| `midnight-did` (umbrella) | blocked | transitively |
| `midnight-did-cli` | `publish = false` | reference demo, not a library |
| `midnight-did-uniffi` | `publish = false` | FFI artifact, shipped as bindings not a crate |
| `midnight-vc-domain` | **publishable now** | none — zero `midnight-*` deps |
| `midnight-vc-runtime` | `publish = false` | `compact-runtime` + `midnight-ledger` unpublished; generated-code artifact |

**Why blocked:** crates.io rejects path-only dependencies. The
workspace path-mounts `third_party/midnight-ledger/*` and
`third_party/compact/runtime-rs` via the devshell; none of those crates
exist on crates.io (verified 2026-08-07). Until the Midnight Foundation
publishes `midnight-ledger`, and `compact-runtime` is published from
[MediaNoxLabs/compact](https://github.com/MediaNoxLabs/compact),
consumers use **git dependencies** on this repo.

**Decision (2026-08-07):** `compact-runtime` stays in the compact repo
(codegen ↔ runtime co-evolve; compact's byte-parity CI compiles
generated fixtures against it — moving it here would invert the repo
dependency). When publishing unblocks, it publishes *from* the compact
fork.

## Release procedure

1. Land everything on `develop`; promote to `rust-codegen` via PR.
2. Bump `workspace.package.version`, finalize the CHANGELOG section.
3. Tag `vX.Y.Z` (signed: `git tag -s`) on `rust-codegen`, push the tag.
4. The [Release workflow](../.github/workflows/release.yml):
   - verifies tag == workspace version, runs fmt/clippy/tests for the
     publishable set,
   - publishes `PUBLISH_CRATES` in dependency order to crates.io
     (skipping versions already published — safe re-run),
   - creates the GitHub Release with generated notes.

Manual dry-run: *Actions → Release → Run workflow* (`dry_run: true`).

## Required setup (GitHub web UI, once)

- Environment `crates-io` (Settings → Environments) with secret
  `CARGO_REGISTRY_TOKEN` (crates.io API token scoped to publish);
  optionally add a required reviewer on the environment as a
  publish-approval gate.

## Extending the publishable set

When an upstream blocker clears: change the blocked crate's workspace
dependency entries from `path` to `version` + published source, append
the crate to `PUBLISH_CRATES` (dependency order), and update the table
above.
