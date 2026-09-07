# Design: add-midnight-vc-families

## Context

Four generated contract modules already live in `crates/midnight-vc-runtime`
(`credentials`, `iso_registry`, `same_holder`, `revocation_registry`), emitted
by `just codegen-vc` — the flake-pinned `compactc --rust --skip-ts` (from
`MediaNoxLabs/compact` @ `afec3cf`) over the vendored submodule
`third_party/midnight-verifiable-credentials` @ `a9f1d451` — and gated by
`just codegen-vc-check`. `doc/adr/0009` fixes the crate-granularity policy;
`doc/adr/0006` records the halo2/wasm constraints that keep codegen targets
out of the wasm32 gate and unpublished.

Today the digital-passport family binding is generated only inside
lace-id-portal (`compiled-digital-passport-contract.nix`, `compactc --skip-zk`
from `yshyn-iohk/compact` @ `a529597` over the upstream tree @ `b68ae4af`,
stored in the nix store, `include!`d via env vars). Verified facts that shape
this design (2026-09-07):

- The family's **entire compile closure** (entry file, family subfiles, the
  whole `packages/core/primitives/credentials` subtree) is **byte-identical**
  between `a9f1d451` and `b68ae4af` — only README/package.json/tests/CHANGELOG
  differ. Generating from the existing vendored pin yields the same contract
  surface lace-id consumes today.
- All 30 exported family circuits are `pure circuit` — `--skip-ts` emits no
  zkir/keys, identical emission shape to the existing four modules.
- Upstream's family tests are **invariant-style** (deterministic fixture →
  length/determinism/alteration-sensitivity assertions), not golden vectors.
- Each generated family `lib.rs` is self-contained: it `include`s the core
  credentials primitive and thereby redeclares the core types — no dependency
  on `midnight-vc-runtime` is needed or wanted.

## Goals / Non-Goals

**Goals:**

- A first-class crate any party can git-depend on for credential-family
  bindings, starting with digital-passport.
- Generated output under the same reproducibility discipline as the existing
  VC modules.
- A family-onboarding path that is purely additive (feature + module + codegen
  entry point).

**Non-Goals:**

- Porting lace-id-portal's issuer adapter (`laceid-credential-digital-passport`:
  holder-binding resolver, sidecar supervisor, signing adapters) — laceid-coupled
  code that stays in that repo.
- Migrating lace-id-portal to consume this crate — separate PR in that repo.
- Binding the `birth` / `birth-secret` families — additive follow-ups.
- Publishing to crates.io — blocked upstream (`compact-runtime`,
  `midnight-ledger`), per `doc/publishing.md`.
- Bumping the vendored submodule pin — unnecessary (closure identity above).

## Decisions

### D1 — New crate `crates/midnight-vc-families`, not a module in `midnight-vc-runtime`

Split-test citation (ADR 0009, reason 2 — independent consumption cadence):
families are pinned by external consumers (lace-id today) and track upstream
family releases (`v0.1.0-rc1`, …), a different lifecycle from the surveyed
core contracts in `vc-runtime`. Layering is clean: `vc-runtime` = core
primitives; `vc-families` = family bindings, each a self-contained core copy.

*Alternative rejected:* a `digital_passport` module inside `midnight-vc-runtime`
— policy-default per ADR 0009, but it buries an externally-pinned, family-versioned
artifact inside the core-contract crate and leaves no charter home for later
families.

### D2 — One cargo feature per family, `default = []`

Kebab-case feature matching the family name (`digital-passport`), gating
`pub mod digital_passport` in `contract/mod.rs`. Default-empty makes every
consumer's opt-in explicit — the surface is prototype-grade and churns
upstream. Features are strictly additive with identical target support
(ADR 0009 feature rules). The heavy closure (`compact-runtime` → halo2 /
arkworks / `midnight-ledger`) is unconditional, so the flags scope which
generated modules compile rather than which dependencies resolve — accepted;
the win is the opt-in signal, not build-weight.

*Alternative rejected:* `default = ["digital-passport"]` — friendlier bare
deps, but asymmetric once later families land default-off, and it weakens the
prototype-opt-in signal. CI compensates with `--all-features` (D6).

### D3 — Codegen folded into `just codegen-vc`, not a mirrored recipe pair

One recipe gains a fifth entry point
(`digital_passport:.../digital-passport-credential.compact`) writing the
header-prepended output to `crates/midnight-vc-families/src/contract/`.
`codegen-vc-check`'s `git diff --exit-code` scope extends to both crates'
generated paths. Same compiler, flags, header pattern, `cargo fmt` pass.

*Alternative rejected:* a `codegen-families` + check pair — independent gating
per crate, but duplicates the recipe body for no benefit while the toolchain,
source pin, and emission flags are shared; folding keeps exactly one place
that knows the compactc invocation.

### D4 — Crate skeleton mirrors `midnight-vc-runtime`

`src/lib.rs` (license header, crate docs stating the all-families charter,
`VERSION` const, `#![warn(missing_docs, …)]`) + `src/contract/mod.rs`
(documented module registry table, `#[cfg(feature)]`-gated modules) +
header-prepended generated file, never hand-edited. Dependencies:
`compact-runtime` plus explicit `midnight-ledger` link-time tripwires
mirroring `vc-runtime`'s rationale (early failure on missing devshell mounts);
the exact tripwire set is whatever the generated `lib.rs` needs at link time —
confirmed during implementation, not guessed. No dependency on
`midnight-vc-runtime` (self-contained generated code). No glob re-exports
across family modules — each module redeclares core types, so a flat re-export
would collide (same reason `vc-runtime` re-exports modules, not contents).

### D5 — Invariant-style smoke tests, no golden vectors

`tests/smoke.rs` with `#![cfg(feature = "digital-passport")]`: a deterministic
fixture ported from the vendored `src/testing/credential-fixtures`, exercising
`pure_circuits` root/commitment circuits — assert determinism, documented
byte length, alteration-sensitivity. Matches upstream's own confidence level;
freezing our output as goldens would bless possibly-wrong semantics, and
codegen-check already catches code drift.

### D6 — Gate wiring with scoped `--all-features`

With `default = []`, a plain `-p midnight-vc-families` gate compiles an empty
crate. justfile gains `families_flags := "-p midnight-vc-families
--all-features"` appended to the fmt/clippy/test/doc recipes (no global
`--all-features`, which could flip other crates' feature-dependent behavior);
`crate_flags`/`coverage_crates` add the crate. `ci.yml` adds the path-filter
entries and per-crate fmt/clippy/test steps mirroring the `vc-runtime` block.
The wasm32 gate excludes the crate (compact-runtime closure, ADR 0006).
`doc/publishing.md` gains the blocked-crate row; `CHANGELOG.md` the entry.

## Risks / Trade-offs

- [Upstream family prototype churn (e.g. the v0.1.0-rc1 ternary-expression
  gap noted in lace-id's flake)] → Vendored pin isolates us; moving the pin is
  a deliberate, surveyed act, and `codegen-vc-check` makes any resulting code
  drift loud.
- [Compiler-pin skew between this repo (`afec3cf`) and lace-id's
  (`a529597`)] → Generated code may differ cosmetically from lace-id's current
  store output; the contract surface is identical (closure byte-identity), and
  `check_runtime_version!` in the generated code hard-fails on runtime skew
  at compile time. lace-id's migration consumes this crate wholesale, so
  their old output becomes irrelevant.
- [Feature-gated modules can silently escape CI gates] → CI and justfile
  recipes compile the crate with `--all-features`; the reproducibility check
  diffs generated files independent of features.
- [Each family module duplicates the core types (self-contained codegen)] →
  Inherent to compactc's per-entry-point emission; accepted and documented —
  consumers must not mix types across family modules or with
  `midnight-vc-runtime`'s core modules.

## Migration Plan

Land on `develop` via the `digital-passport-lib` branch. No rollout: the crate
is additive; no existing crate or generated file changes. Rollback = revert
the merge. lace-id-portal migrates in a follow-up PR in that repo (drop
`compiled-digital-passport-contract.nix` + the env-var `include!`, add a git
dependency); until then their nix path is unaffected.

## Open Questions

- Exact `midnight-ledger` tripwire dependency set for the new crate —
  discovered from the generated `lib.rs` link requirements during
  implementation; does not affect surface, specs, or tasks.
