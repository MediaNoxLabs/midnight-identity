<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# ADR 0009 — Crate granularity policy

**Status:** accepted · **Date:** 2026-08-07
**Driver:** maintainer directive — "I don't want plenty of crates, and at
the same time I don't want to manage the monolith; some crates should be
reusable and probably have a binding for uniffi or wasm."

## Context

The workspace is at 9 crates (~16k LoC) and growing: the backlog proposes
a Jubjub-Schnorr suite (#7), a provider-generalization crate (#8), VC
core crates (#13), and eventually WASM bindings. Without a policy, each
addition is an ad-hoc judgment call, and both failure modes — crate
sprawl and a monolith — are one lazy decision away.

We researched how production Rust stacks with the same constraints
(reusable cores + UniFFI + WASM bindings) draw crate boundaries:
mozilla/application-services (UniFFI's home; megazord pattern),
matrix-rust-sdk (core/bindings split, per-platform storage crates),
zcash/librustzcash (layering by stability, heavy deps in leaves), and —
closest to us — iotaledger/identity.rs and spruceid/ssi (SSI stacks:
spec-boundary cores, one umbrella facade, bindings as leaf crates).
Plus the Cargo Book's feature-additivity rule and matklad's
"Large Rust Workspaces". Full citations at the bottom.

## Decision — the split test

**A new crate must cite at least one of these six reasons in its PR
description. No reason → it's a module. A "nice conceptual boundary" is
not a reason.**

1. **Target/dependency-weight isolation.** The code drags deps a
   consumer class must not pay for (halo2/arkworks, reqwest/tokio-net,
   sqlite) or breaks a compilation target another crate must keep
   (wasm32). *Examples here: `runtime` (ADR 0006), `indexer`.*
2. **Independent semver/publishing cadence.** It ships to consumers on
   its own schedule (crates.io, npm, a git tag other repos pin).
   *Example: `domain` (publishable today, spec-stable).*
3. **The alternative would be a mutually exclusive cargo feature.**
   Features must be additive (feature unification takes the union across
   the graph); "either/or" functionality splits into packages.
4. **Binding-technology boundary.** uniffi / wasm-bindgen / napi / pyo3
   machinery lives only in a dedicated **leaf** crate; cores never
   reference binding tech. *Example: `uniffi`.*
5. **Compile-time blast radius.** The split demonstrably parallelizes
   builds or shrinks the incremental-rebuild scope (matrix-rust-sdk
   keeps internal crates purely for this). Cite numbers, not vibes.
6. **Pluggable implementation behind a trait.** Trait in the core crate,
   each backend its own crate (`Backend` in `runtime` →
   `IndexerBackend` in `indexer`; zcash_client_backend →
   zcash_client_sqlite is the reference shape).

**Modules, not crates**, when target support, deps, release cadence, and
consumers all match the parent — e.g. `method::holder_binding` and
`runtime::state_decode` (both added as modules under this rule).

**Feature flags** only for strictly additive extras with identical
target support; never for platform switching (use
`[target.'cfg(...)'.dependencies]`) and never gating public fields or
trait methods.

## Binding architecture (forward-looking, binding on #7/#13/wasm work)

- **Cores stay binding-free.** No `uniffi::*` or `wasm_bindgen`
  anywhere outside dedicated leaf crates.
- **One UniFFI leaf per shipped library** (megazord pattern): mobile
  gets ONE cdylib/xcframework, not one per crate. `midnight-did-uniffi`
  is that leaf; when VC crates land, their FFI surface joins the same
  leaf (UniFFI library mode generates bindings for all UniFFIed crates
  linked into one library) rather than spawning `-uniffi` siblings.
- **WASM = a new leaf crate** (`midnight-identity-wasm`, when it
  happens), wrapping the wasm-clean cores with wasm-bindgen — never a
  `wasm` feature on the cores. UniFFI and WASM leaves are siblings over
  the same cores (the matrix-rust-sdk shape).
- **Wasm-clean core set** is enforced by CI's wasm32 build job. Today:
  `domain`. Restoring `method` (and the domain-shape slice of `api`) to
  that set is issue #12 and *raises the value of every future binding*.
- **Publishing is deliberate** (doc/publishing.md): internal/blocked
  crates stay `publish = false`; the umbrella (`midnight-did`) is the
  one-dependency entry point for app consumers and stays a pure
  re-export.

## Current inventory against the test (2026-08-07)

| Crate | LoC | Split reason | Verdict |
|---|---|---|---|
| `midnight-did-domain` | 2852 | 1 (wasm-clean, zero midnight deps) + 2 (publishable) | keep |
| `midnight-did-method` | 1881 | 2 (VC-consumable seam) — currently fails rule 1 (drags base-crypto); #12 fixes | keep |
| `midnight-did-api` | 2769 | 6 (operations over `Backend`) | keep |
| `midnight-did-runtime` | 6513 | 1 (halo2/arkworks; ADR 0006) + codegen target | keep |
| `midnight-did-indexer` | 270 | 1 (reqwest/tokio) + 6 (backend impl) | keep; **candidate to merge into the #8 providers crate when it exists** — two trait-backend crates this small don't both survive the test |
| `midnight-did` (umbrella) | 70 | facade (identity.rs/ssi pattern) | keep |
| `midnight-did-uniffi` | 639 | 4 (binding leaf) | keep |
| `midnight-did-resolver` | 574 | service **bin** leaf | keep |
| `midnight-did-cli` | 789 | demo **bin** leaf | keep |

Nine crates, every one passing the test — comfortably inside the norms
of identity.rs (~12 lib crates) and ssi (~19) at comparable scope. The
policy's job is keeping it that way.

## Consequences for the open backlog

- **#7 `midnight-jubjub-schnorr`**: new crate justified — rules 1
  (wasm-clean crypto, no runtime deps) + 2 (own cadence; both DID and VC
  layers consume it; mirrors the upstream TS cohort's separate package).
- **#8 providers**: new crate justified by rule 6 — but it **absorbs
  `midnight-did-indexer`** rather than adding a sibling; net crate
  count unchanged.
- **#13 VC cores**: split by spec boundary like identity.rs (a
  credential-model crate + a generated-bindings crate), NOT per
  credential family — families are downstream examples, not
  infrastructure. FFI joins the existing uniffi leaf (megazord rule).
- Anything smaller (hash domains, codecs, mappers) = modules in
  existing crates.

## References

- application-services megazords: <https://mozilla.github.io/application-services/book/design/megazords.html>
- matrix-rust-sdk (core/bindings/storage layout): <https://github.com/matrix-org/matrix-rust-sdk>; wasm sibling: <https://github.com/matrix-org/matrix-sdk-crypto-wasm>
- librustzcash layering: <https://github.com/zcash/librustzcash>
- identity.rs (SSI reference layout): <https://github.com/iotaledger/identity.rs>; spruceid/ssi: <https://github.com/spruceid/ssi>
- matklad, *Large Rust Workspaces*: <https://matklad.github.io/2021/08/22/large-rust-workspaces.html>
- Cargo Book, feature additivity: <https://doc.rust-lang.org/cargo/reference/features.html#mutually-exclusive-features>
- Effective Rust, Item 26 (features): <https://www.lurklurk.org/effective-rust/features.html>
- UniFFI library mode / bindgen bin crate: <https://mozilla.github.io/uniffi-rs/latest/tutorial/foreign_language_bindings.html>
- rustwasm, adding wasm support: <https://rustwasm.github.io/book/reference/add-wasm-support-to-crate.html>
