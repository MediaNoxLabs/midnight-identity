# Design: track-digital-passport-standalone

## Context

The unmerged `digital-passport-lib` branch (change
`2026-09-08-add-midnight-vc-families`, archived on the branch) generates the
family binding from the vendored monorepo pin `a9f1d451`. One day after that
work, upstream reset the monorepo (`754b2af`, PR #551, 2026-09-08): the whole
`packages/prototypes/` tree and `packages/core/primitives/credentials` were
deleted (289,898 lines, 724 files; ADR-0016 "core-only specification"). The
family's graduated home is the standalone repo, release `v0.1.0-rc1` (tag
commit `bf2b608`, 2026-09-01; `src/` identical to `main`). Verified facts
that shape this design (2026-09-09):

- The standalone repo ships **only the six family `.compact` files**; the
  core is consumed from the published npm package
  `@midnight-ntwrk/credential-compact@0.1.0-rc3` (tarball immutable per
  version; ships `.compact` sources under `src/` and `dist/`). The entry's
  `include "../core-compact-staging/credentials"` targets a gitignored
  directory populated at build time by copying from that package
  (`scripts/stage-core-compact.mjs`, `.compact` files only). Without staging,
  the include dangles.
- rc3 core ≈ monorepo `a9f1d451` core minus ~25 lines of status-attestation
  circuits in `proofs.compact`; `same-holder` also drifted. The family's own
  sources diverged beyond headers: request format pinned to `version: 1`, and
  the age predicate reworked around a `DigitalPassportCivilDate`
  witness-decomposition verified by exact reconstruction.
- The standalone repo's own toolchain pins upstream compact compiler
  `0.31.1` and npm `compact-runtime@0.16.0`; our codegen uses the flake-pinned
  fork (MediaNoxLabs `codegen-rust` @ `afec3cf`, `compact-runtime` 0.16.100)
  — the only compiler with `--rust`.
- The standalone family package is **not on npm** (404); `v0.1.0-rc1` was
  published as a GitHub Release (npm-style `.tgz` + `SHA256SUMS`) pending an
  npm token.

## Goals / Non-Goals

**Goals:**

- The family binding tracks the standalone repository's release tag, not a
  dead pre-reset snapshot.
- Codegen reproducibility under the existing gate (`codegen-vc-check`), with
  integrity-pinned core fetch.
- The crate's generated surface matches what the graduated upstream actually
  defines (lace-id-portal will git-depend on this crate directly; no
  monorepo-shape constraint remains).

**Non-Goals:**

- Migrating the four `midnight-vc-runtime` core modules off the frozen
  `a9f1d451` pin (recorded follow-up below).
- Publishing to crates.io; migrating lace-id-portal (their repo's PR).
- Editing the archived `2026-09-08-add-midnight-vc-families` change — it is
  the historical record; this change is the correction.

## Decisions

### D1 — New submodule pinned to the release tag; tarball-only rejected

`third_party/midnight-verifiable-credential-digital-passport`, `branch =
main`, rev = `v0.1.0-rc1` (`bf2b608`). Consistent with the repo's two
existing submodules; pointer bumps are reviewable, diffable pins; CI's
`submodules: recursive` fetches it for free.

*Alternative rejected:* sourcing the family from the `v0.1.0-rc1` GitHub
Release `.tgz` (it does ship `src/**/*.compact`). Viable and uniform with the
core fetch, but GitHub release assets are maintainer-deletable (weaker than a
git tag), and a pin bump would become a hash edit instead of a diffable
pointer. Chosen for consistency; the tarball channel remains a documented
fallback.

### D2 — Core staged at codegen time from the npm tarball; no node toolchain

`just codegen-vc` curls
`registry.npmjs.org/@midnight-ntwrk/credential-compact/-/credential-compact-0.1.0-rc3.tgz`,
verifies it against a sha256 recorded in the justfile, caches it under
`target-gen/` (repeat runs offline), and copies `dist/credentials.compact` +
`dist/credentials/` into the submodule's `core-compact-staging/` — mirroring
`stage-core-compact.mjs` semantics (`.compact` files only). The npm registry
is the core's only release channel; the standalone repo deliberately consumes
published semver rather than vendoring core in-tree.

*Alternatives rejected:* adding `nodejs` to the devshell to run
`npm pack`/the upstream staging script (new toolchain dependency, or a full
`pnpm install` of the family workspace, for a plain file copy); vendoring the
staged core into `third_party` as a checked-in copy (hermetic but a manually
re-vendored copy); reusing the monorepo pin's core subtree (dead upstream, and
`proofs.compact`/`same-holder.compact` already drifted from rc3).

### D3 — Fork compiler stays the compiler of record

Regeneration keeps the flake-pinned `compactc --rust --skip-ts` (same
compiler/flags as the other five entry points). Upstream parity is best-effort
(the standalone targets 0.31.1; the fork is 0.31.1xx-era) and is gated by the
smoke tests, not asserted. `check_runtime_version!` binds the output to our
`compact-runtime`, which is internally consistent.

### D4 — Surface churn accepted, no shims

The regenerated module will carry `DigitalPassportCivilDate` +
`assertCivilDateMatchesEpochDays`, `version: 1` request pinning, and rc3-based
core redeclarations (without the monorepo pin's status-attestation circuits).
The crate is `publish = false`, feature-gated, consumerless — the cheapest
moment to take the churn is now.

### D5 — Smoke suite: port three, add one

Port the three existing tests to the standalone `./testing` fixtures
(claim-root, per-field commitments, null commitment). Add a civil-date
witness test: valid decomposition accepted; corrupted quotient field
rejected; mismatched day number rejected. Matches the new spec scenario.

### D6 — Deliverable shape: one corrective commit + this change

One new commit on top of the unmerged branch (no history rewrite; the branch
is already pushed), carrying this change's artifacts and the implementation.
CI: add the new submodule's path filter to `ci.yml`; no new job; the tarball
fetch lives inside `just codegen-vc` so local and CI run the identical path.
`CHANGELOG.md` entry; `.gitmodules` + justfile + regenerated file + tests.

## Risks / Trade-offs

- [Fork compiler vs upstream-0.31.1-targeted source] → Verified by compiling
  during implementation; gated thereafter by smoke tests + `codegen-vc-check`.
  If the fork cannot compile the reworked helpers, the fallback is pinning a
  later fork rev (a flake bump, existing pattern).
- [GitHub/npm availability at codegen time] → Cached tarball under
  `target-gen/`; sha256 pin makes the fetch deterministic; the gate compares
  against committed output, so a fetch failure is loud, not silent.
- [Core rc3 ≠ monorepo core (status-attestation dropped)] → Expected,
  accepted churn in the redeclared core types inside the family module;
  documented in the generated-file header provenance.
- [Frozen `a9f1d451` pin for the four core modules] → Submodules are immutable
  snapshots; the drift gate keeps them honest. Follow-up recorded below, not
  urgent.

## Migration Plan

Land on the unmerged `digital-passport-lib` branch as an additional commit.
No rollout: the crate is additive and consumerless. Rollback = revert the
commit. lace-id-portal migrates in their own follow-up PR (drop their
nix-store derivation; add a git dependency).

## Recorded Follow-up (out of scope)

The monorepo reset orphaned the four `midnight-vc-runtime` sources. Split:

1. **Re-core `credentials` + `same_holder`** from `@midnight-ntwrk/credential-compact`
   (targets exist today: `src/credentials/*` and `src/holder-binding/same-holder.compact`).
2. **Decide the fate of `iso_registry` + `revocation_registry`** — no new home
   exists anywhere (checked: org repos, npm, `credential-compact` contents;
   ADR-0016 enumerates no successors). Options: keep on the frozen pin as
   historical artifacts, or deprecate/remove. Nothing to migrate *to*.

## Open Questions

(none — resolved in the design review preceding this change: branch-history
mechanics, vendoring mechanism, pin policy, churn acceptance, compiler,
scope, hermeticity, lace-id coordination, CI wiring.)
