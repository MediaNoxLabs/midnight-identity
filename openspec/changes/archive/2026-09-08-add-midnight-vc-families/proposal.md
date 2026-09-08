# Proposal: add-midnight-vc-families

## Why

The Rust binding for the digital-passport credential exists only inside
lace-id-portal's nix orchestration: a derivation runs `compactc --rust` over the
family contract and stores the generated crate in the nix store, where the
issuer consumes it through env-var `include!` plumbing. No other party can use
that binding. Hosting it here as a first-class, feature-gated workspace crate
makes the credential family available to any consumer via a plain git
dependency, under this repo's codegen reproducibility gates.

## What Changes

- **New crate `crates/midnight-vc-families`** — the credential-family codegen
  target. Charter: hold the Rust bindings for *all* credential families; lands
  with the **digital-passport** family only (birth / birth-secret are additive
  follow-ups). One cargo feature per family (`digital-passport`),
  `default = []`; `publish = false` (blocked on `compact-runtime` /
  `midnight-ledger` upstream, git-dep consumption per `doc/publishing.md`);
  `version.workspace = true`.
- **Codegen**: `just codegen-vc` gains a fifth entry point compiling
  `packages/prototypes/credential-families/digital-passport/src/digital-passport-credential.compact`
  from the existing vendored submodule pin (`a9f1d451` — its compile closure is
  byte-identical to lace-id-portal's `b68ae4af`, verified 2026-09-07).
  `codegen-vc-check` gates generated files in both crates.
- **Tests**: integration smoke tests (invariant-style, deterministic fixtures
  ported from the vendored upstream `src/testing/`) proving the generated
  circuits compute — the existing four `midnight-vc-runtime` modules have no
  circuit tests; this crate's purpose is external consumption, so a
  compute-proof is warranted.
- **Wiring**: justfile gate recipes, `ci.yml` per-crate steps, wasm-gate
  exclusion (compact-runtime closure), `CHANGELOG.md` entry, `doc/publishing.md`
  table row.

Out of scope: migrating lace-id-portal off its nix-store derivation (separate
PR in that repo; their path keeps working until then — the generated surface
here is identical by construction).

## Capabilities

### New Capabilities

- `vc-families`: the credential-family binding crate — per-family feature-gated
  generated modules, their codegen workflow and reproducibility gate, and the
  smoke-test contract for generated circuits.

### Modified Capabilities

(none — `midnight-vc-runtime` and the four core-contract modules are untouched;
the codegen recipe extension is covered under the new capability's workflow
requirements.)

## Impact

- **Workspace**: `Cargo.toml` members += `crates/midnight-vc-families`; new
  workspace dependency entry (sibling of `midnight-vc-runtime`, no dependency
  on it — each generated family `lib.rs` is self-contained).
- **Tooling**: `justfile` (`codegen-vc`, `codegen-vc-check`, crate gate flags),
  `.github/workflows/ci.yml` (path filters + fmt/clippy/test steps).
- **Docs**: `CHANGELOG.md`, `doc/publishing.md`, crate-level rustdoc.
- **No breaking changes**: additive crate; no existing crate's API, features,
  or generated output changes.
