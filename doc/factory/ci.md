<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Proportional validation and caches

[`scripts/ci/target-plan.mjs`](../../scripts/ci/target-plan.mjs) is the
path-to-gate authority. Unknown paths and unavailable diffs fail closed to the
full production plan. Pull requests use `production-ready`; prototype plans
are local-only. Pushes to `develop`/`rust-codegen`, scheduled audits, and
manual full runs execute the complete matrix.

| Change | Required targets |
| --- | --- |
| Docs, factory policy/tooling, or CI only | `policy` only; no Rust/Nix closure |
| Rust crate | `policy`, focused format/lint/compile, affected unit tests, coverage |
| Wasm-clean domain crate | Rust targets plus `wasm` |
| DID Compact input/generated artifacts | Rust targets plus `did-codegen` |
| VC Compact input/generated artifacts | Rust targets plus `vc-codegen` |
| Cargo/Nix/lock/toolchain or unknown | Full production matrix |
| Manual/integration/release | Full production matrix |
| Scheduled | Full production matrix plus factory audit |

The planner expands a changed crate to known first-party dependents so focused
testing still exercises compatibility. Pull requests into `rust-codegen` or a
future `milestone-x.y.z` branch force the full release plan. The planner emits
a stable GitHub output contract consumed by `.github/workflows/ci.yml`; its
contract tests are part of the always-on policy lane.

## Cache policy

Nix uses only public substituters configured by the flake and the standard Nix
installer. No FlakeHub token or account is required. PR #55 owns the split
between the light Rust shell and the Compact-capable shell; this foundation
does not duplicate it.

Cargo caching is intentionally bounded to registry indexes, registry archives,
and Git checkout databases under an exact OS plus `Cargo.lock` key. Worktree
`target/` directories are never uploaded or shared. Each worktree owns its
mutable target tree, and cache inventory/disk growth is visible in the
periodic factory audit.

## Local use

```bash
node scripts/ci/target-plan.mjs \
  --base "$(git merge-base HEAD origin/develop)" \
  --head HEAD \
  --delivery-profile production-ready

node scripts/factory/check.mjs
```

Run only the emitted commands/targets. Compact drift checks require the full
Nix shell; ordinary Rust checks use `nix develop .#rust` after PR #55.
