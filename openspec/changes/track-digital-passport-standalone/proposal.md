# Proposal: track-digital-passport-standalone

## Why

The upstream `midnight-verifiable-credentials` monorepo was reset on
2026-09-08 (`754b2af`, PR #551): the entire `packages/prototypes/` tree —
including the digital-passport family the unmerged `add-midnight-vc-families`
change generates from — and `packages/core/primitives/credentials` were
deleted; families now live in independently versioned repositories. The
family's graduated home is
[`midnightntwrk/midnight-verifiable-credential-digital-passport`](https://github.com/midnightntwrk/midnight-verifiable-credential-digital-passport),
which has already diverged semantically (reworked age predicate with a
`DigitalPassportCivilDate` witness, request format pinned to `version: 1`).
`lace-id-portal` will consume this crate via git dependency, so the crate must
track the live upstream rather than a dead pre-reset snapshot.

## What Changes

- **New submodule** `third_party/midnight-verifiable-credential-digital-passport`
  (`branch = main`, rev pinned to tag `v0.1.0-rc1`, commit `bf2b608`) — the
  family's source of record. The four core-contract modules in
  `midnight-vc-runtime` stay on the frozen `midnight-verifiable-credentials`
  pin `a9f1d451`.
- **Core staging at codegen time**: `just codegen-vc` fetches
  `@midnight-ntwrk/credential-compact@0.1.0-rc3` from the npm registry as a
  tarball (curl, sha256 pinned in the justfile, cached under `target-gen/`)
  and copies its `.compact` sources into the submodule's gitignored
  `core-compact-staging/`, mirroring upstream's `stage-core-compact.mjs`
  semantics. The standalone repo does not vendor the core; without staging the
  entry point's include dangles.
- **Regeneration**: the fifth `codegen-vc` entry point moves to the standalone
  repo's entry file; `crates/midnight-vc-families/src/contract/digital_passport.rs`
  is regenerated with the flake-pinned fork `compactc --rust --skip-ts`
  (unchanged). Surface churn is accepted with no compatibility shims — new
  `DigitalPassportCivilDate` witness struct + `assertCivilDateMatchesEpochDays`
  circuit, request-version pinning, and slightly changed core redeclarations
  (rc3 core drops the status-attestation circuits present in the monorepo pin).
- **Smoke tests**: port the three existing tests to the standalone repo's
  `./testing` fixtures and add a fourth covering the civil-date witness
  verification (valid decomposition passes; corrupted quotient / wrong day
  fail).
- **CI**: `.github/workflows/ci.yml` gains the new submodule's path filter; no
  new job (`submodules: recursive` already fetches it; the tarball fetch runs
  inside `just codegen-vc` so local and CI run the identical path).
- **Documentation**: `CHANGELOG.md` entry; the archived
  `2026-09-08-add-midnight-vc-families` change stays untouched as the
  historical record — this change is the correction.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `vc-families`: the generated-surface fidelity requirement's source
  definition changes — the family binding no longer shares a vendored source
  with the workspace's other VC bindings; it is now the pinned compiler over
  the pinned standalone-family tag plus the pinned `credential-compact` core
  package. The smoke-contract requirement gains a scenario for the
  witness-verified age-predicate circuit.

## Impact

- **Vendoring**: `.gitmodules` (+1 submodule); the
  `third_party/midnight-verifiable-credentials` submodule is retained (four
  core modules still source from it).
- **Tooling**: `justfile` (`codegen-vc` entry list, tarball fetch + staging
  step with sha256 pin), `.github/workflows/ci.yml` (path filters).
- **Code**: `crates/midnight-vc-families/src/contract/digital_passport.rs`
  (regenerated) and `tests/` (fixtures + new test). No API compatibility
  constraints: the crate is `publish = false`, feature-gated
  (`digital-passport`), and has no consumers yet — lace-id-portal migrates to
  it in their own follow-up.
- **Known risk**: the flake-pinned fork compiler (0.31.1xx-era) compiling
  source targeted at upstream 0.31.1 — verified by compiling, gated by the
  smoke tests and `codegen-vc-check`.
