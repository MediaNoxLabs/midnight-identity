<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# AI Software Factory

The factory is a small, repository-owned delivery contract for reusable
Midnight libraries. It coordinates issues, isolated worktrees, proportional
validation, draft pull requests, exact-head evidence, and safe cleanup. It
does not decide product policy or replace human ownership of integration and
release branches.

## Sources of truth

| Concern | Authority |
| --- | --- |
| Always-loaded repository boundary and safety rules | [`AGENT.md`](../../AGENT.md) |
| Branches, commits, profiles, and handoff | [`delivery.md`](delivery.md) |
| Path-to-gate routing and cache policy | [`ci.md`](ci.md) |
| Pi supervisor and worktree lifecycle | [`supervisor.md`](supervisor.md) |
| Per-run evidence and periodic audit | [`metrics.md`](metrics.md) |
| Architecture | [`../architecture.md`](../architecture.md) and [`../adr/`](../adr/) |

## Boundaries

Candidate library assets include Midnight DID crates/runtime, reusable VC
components, Passport Vault Compact source and generated/runtime artifacts,
Midnight bindings/test utilities, and wallet synchronization/runtime code only
after two consumers prove the seam. Oxid and lace-id-portal stay read-only
consumers during library work unless separately authorized. Chain-neutral
protocols and primitives stay in `sdk-rust`.

The initial foundation deliberately excludes a durable coordination service,
database, canary environment, autonomous `develop`/`rust-codegen` merge, and
application deployment machinery. GitHub issues and pull requests remain the
durable planning and evidence surfaces.

## Foundation sequence

1. Stabilize the foundation and hand its `develop` merge to a human.
2. Stabilize and merge existing Midnight DID PR #53.
3. Record its immutable consumer commit for Oxid.
4. Stabilize and merge Passport Vault source PR #52.
5. Record its immutable consumer commit for Oxid.
6. Evaluate lace-id-portal adoption only where concrete duplication exists.

Keep each repository stable before moving the next consumer. Do not turn this
sequence into a big-bang migration.
