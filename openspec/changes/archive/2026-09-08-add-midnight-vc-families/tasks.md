# Tasks: add-midnight-vc-families

## 1. Crate scaffold

- [x] 1.1 Add `crates/midnight-vc-families` to workspace `members` and a
  workspace.dependencies entry (sibling of `midnight-vc-runtime`); create
  `Cargo.toml` (`version.workspace`, `publish = false`, `description` naming
  the family-charter) with feature `digital-passport`, `default = []`, deps
  `compact-runtime` + the `midnight-ledger` tripwires; verify
  `cargo metadata` resolves and `cargo build -p midnight-vc-families`
  (featureless, empty crate) succeeds
- [x] 1.2 Create `src/lib.rs` (license header, crate docs stating the
  all-families charter and per-family feature gating, `VERSION` const,
  `pub mod contract`) and `src/contract/mod.rs` (documented module registry
  table, `#[cfg(feature = "digital-passport")] pub mod digital_passport;`);
  verify `cargo doc -p midnight-vc-families` renders without warnings

## 2. Codegen extension

- [x] 2.1 Extend `just codegen-vc` with the fifth entry point
  (`digital_passport` → `packages/prototypes/credential-families/digital-passport/src/digital-passport-credential.compact`)
  writing the GENERATED-header file to
  `crates/midnight-vc-families/src/contract/digital_passport.rs`; run
  `nix develop -c just codegen-vc` and verify the four existing modules
  regenerate byte-identically (`git diff` shows only the new file) and the
  new file carries the header + `compact_runtime::check_runtime_version!`
- [x] 2.2 Extend `codegen-vc-check`'s `git diff --exit-code` scope to
  `crates/midnight-vc-families/src/contract`; verify `just codegen-vc-check`
  passes twice in a row on a clean tree
- [x] 2.3 Build with `cargo build -p midnight-vc-families --features
  digital-passport` and fix the link-time `midnight-ledger` tripwire
  dependency set per the actual generated-code requirements (design Open
  Question); verify `cargo clippy -p midnight-vc-families --all-features
  --all-targets -- -D warnings` is clean

## 3. Smoke tests

- [x] 3.1 Port the deterministic digital-passport fixture from the vendored
  submodule (`src/testing/` credential fixtures) into `tests/` support code;
  verify the fixture builds the claim-commitments input without hardcoded
  golden outputs
- [x] 3.2 Write `tests/smoke.rs` gated by `#![cfg(feature = "digital-passport")]`
  exercising `pure_circuits` claim-root + one commitment circuit: assert
  determinism (same fixture → same root), documented byte length, and
  alteration-sensitivity (one altered commitment → different root); verify
  `cargo test -p midnight-vc-families --all-features` passes and a bare
  `cargo test -p midnight-vc-families` compiles zero tests without failing

## 4. Gates & wiring

- [x] 4.1 justfile: append `families_flags := "-p midnight-vc-families
  --all-features"` to the fmt/clippy/test/doc recipes (no global
  `--all-features`), add the crate to `crate_flags` and `coverage_crates`,
  and add `contract/digital_passport\.rs` to `coverage_exclude`; verify
  `just fmt-check lint test` pass
- [x] 4.2 `.github/workflows/ci.yml`: add `crates/midnight-vc-families/**`
  path filters and fmt/clippy/test steps with `--all-features` mirroring the
  `midnight-vc-runtime` block (no wasm32 gate for this crate); verify the
  workflow file parses (e.g. `act --list` or YAML lint) and the new steps
  reference the right recipe/commands
- [x] 4.3 Add the blocked-crate row to `doc/publishing.md` (blocker:
  `compact-runtime` + `midnight-ledger` unpublished; git-dep consumption) and
  the `CHANGELOG.md` entry; verify doc references to the new crate are
  consistent (`grep -r midnight-vc-families doc/ CHANGELOG.md`)

## 5. Final verification

- [x] 5.1 Full gate sweep on a clean tree: `just ci` (fmt-check, lint, build,
  test, coverage gates) plus `just codegen-vc-check`; verify everything is
  green and `git status` shows no generated-file drift
