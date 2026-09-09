# Tasks: track-digital-passport-standalone

## 1. Vendoring

- [ ] 1.1 Add submodule `third_party/midnight-verifiable-credential-digital-passport`
  (`branch = main`) pinned to `v0.1.0-rc1` (`bf2b608`); verify
  `git submodule status` reports the pinned rev and `git submodule update
  --init` fetches it cleanly
- [ ] 1.2 Record the pinned `credential-compact-0.1.0-rc3.tgz` sha256 (fetch
  once, hash it); verify the digest matches the upstream release artifacts or
  npm integrity data

## 2. Codegen

- [ ] 2.1 Extend `just codegen-vc` with the core-staging preamble: fetch the
  tarball by URL (cached under `target-gen/`, skip fetch when cached), verify
  against the recorded sha256 (fail loudly on mismatch), extract, and copy
  `dist/credentials.compact` + `dist/credentials/` into the submodule's
  `core-compact-staging/` mirroring `stage-core-compact.mjs` semantics;
  verify a second run is offline and idempotent
- [ ] 2.2 Repoint the fifth entry point to
  `third_party/midnight-verifiable-credential-digital-passport/packages/midnight-verifiable-credential-digital-passport/src/digital-passport-credential.compact`
  and update the prepended header to record entry point + source tag + core
  package version; run `just codegen-vc` and verify the four core modules
  regenerate byte-identically (`git diff` shows only `digital_passport.rs`
  changed) and the header carries the provenance
- [ ] 2.3 Verify the regenerated file compiles:
  `cargo build -p midnight-vc-families --features digital-passport`; if the
  fork compiler rejects the reworked `helpers.compact`, stop and record the
  fork-rev bump needed (design D3 fallback) before proceeding
- [ ] 2.4 Verify `just codegen-vc-check` passes twice in a row on a clean
  tree (its `git diff --exit-code` scope already covers the crate)

## 3. Smoke tests

- [ ] 3.1 Port the fixture support (`tests/support/`) to the standalone
  repo's `./testing` sources; verify the fixture builds the claim-commitments
  and civil-date witness inputs without hardcoded golden outputs
- [ ] 3.2 Reconcile the three existing tests (claim-root determinism/size/
  alteration-sensitivity, per-field commitments, null-commitment sentinel)
  with the regenerated surface; verify
  `cargo test -p midnight-vc-families --all-features` passes them
- [ ] 3.3 Add the civil-date witness test (valid decomposition accepted;
  corrupted quotient field rejected; mismatched day number rejected — the new
  spec scenario); verify it fails when the witness is corrupted as asserted
  and passes on the valid fixture

## 4. Wiring & docs

- [ ] 4.1 `.github/workflows/ci.yml`: add
  `third_party/midnight-verifiable-credential-digital-passport/**` to the
  `pull_request` and `push` path filters; verify the workflow file parses and
  no new job is added (the `codegen-vc-check` job covers the swap via
  `submodules: recursive` + the in-recipe tarball fetch)
- [ ] 4.2 Add the `CHANGELOG.md` entry (motivation: upstream reset `754b2af`
  deleted the monorepo family + core trees; new source pin, staging mechanism,
  surface churn summary); verify `grep -r digital-passport CHANGELOG.md
  README.md doc/` references stay consistent
- [ ] 4.3 Confirm the archived `2026-09-08-add-midnight-vc-families` change
  is untouched (`git status` shows no edits under
  `openspec/changes/archive/`)

## 5. Final verification

- [ ] 5.1 Full gate sweep on a clean tree: `just fmt-check lint build test`
  plus `just codegen-vc-check` and `just codegen-check`; verify everything is
  green and `git status` shows no generated-file drift beyond the intended
  `digital_passport.rs` change
