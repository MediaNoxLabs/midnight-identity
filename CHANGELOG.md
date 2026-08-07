<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Changelog

All notable changes to the `midnight-identity` workspace are recorded
here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [SemVer](https://semver.org/).

## [Unreleased]

### Added

- **LiveBackend read path (issue #4)** — resolution can now go live:
  - `midnight-did-runtime::state_decode`: deserialize indexer
    `contractAction(address){state}` bytes (`tagged_deserialize` of
    `contract-state[v6]`) and project the nested `[4][15]`-chunked
    ledger layout into `DidLedgerSnapshot` — scalars via the generated
    `ledger()` accessors, Map/Set fields via a direct alignment-encoded
    atom walk (round-trip tested against chain-shaped fixtures).
  - New crate `midnight-did-indexer`: minimal GraphQL client issuing
    the same `CONTRACT_STATE_QUERY` as the TS
    `indexer-public-data-provider` 4.x, plus `IndexerBackend` — a
    read-only `Backend` (`submit_tx` → `ReadOnly`).
  - Known codegen follow-ups discovered en route (tracked on the
    compact PR): the constructor's initial-state scaffold is flat
    rather than `[4][15]`-chunked (A29), and the generated `id()`
    accessor decodes via field-repr where cells are alignment-encoded.
  - **Live acceptance passed (2026-08-07)**: a 0.5.0 DID deployed via
    the TS reference stack resolves from pure Rust
    (`cargo run -p midnight-did-indexer --example live_resolve`) with
    **byte-identical metadata** and a document identical to the
    upstream `MidnightDIDResolver` output modulo TS's `null`-for-empty
    quirk. Harness: `infra/standalone.yml` +
    `doc/integration-standalone.md`.

- **Rust DID resolver service (issue #5)** — new crate
  `midnight-did-resolver` (axum): `GET /resolve/{did}`,
  `POST /resolve`, `/health`, `/ready`; W3C resolution envelope with
  the TS service's status mapping (`notFound` 404,
  `invalidDid`/`networkMismatch` 400, `internalError` 500) but
  **typed** error classification (no string matching); SSRF endpoint
  policy for per-request `indexerUrl` overrides; env config
  (`RESOLVER_HOST/PORT`, `MIDNIGHT_INDEXER_HTTP_URL`,
  `MIDNIGHT_NETWORK`, `RESOLVER_TIMEOUT_MS`). Route tests run against
  a mock indexer serving **real captured chain state**. Live
  head-to-head vs the TS `did-resolver-service` on the same DID:
  `didDocumentMetadata` + `didResolutionMetadata` identical, document
  identical modulo the TS `null`-for-empty quirk (#15); all error legs
  byte-matched.

### Fixed

- DID document `@context` updated to the 0.5.0 spelling
  (`https://w3id.org/security/jwk/v1`, was the stale
  `vc-jws-2020` URL) in `resolution.rs`, `offchain.rs`, and the 13
  API JSON fixtures — found by the live parity diff.

- Repository moved to `MediaNoxLabs/midnight-identity` (from
  `yshyn-iohk/midnight-did-rs`); all self-references, Cargo `repository`/
  `homepage` metadata, and the compact flake input
  (`github:MediaNoxLabs/compact/codegen-rust`) repointed.
- Public-repo hardening, mirroring `midnightntwrk/midnight-did`:
  `CODE_OF_CONDUCT.md` (Contributor Covenant 2.1), `CODEOWNERS`, issue
  templates (bug / feature / documentation), PR template with a
  contract-surface checklist, grouped weekly `dependabot.yml`
  (cargo + github-actions, 7-day cooldown, majors excluded), and
  `.envrc` commit/tag-signing defaults.
- Coverage gating: `cargo-llvm-cov` in the devshell (`llvm-tools`
  toolchain component), `just coverage` / `coverage-gate` /
  `coverage-lcov` recipes with a 75% line floor (baseline 77.18%), a CI
  `coverage` job uploading LCOV artifacts, and `coverage-gate` wired
  into `just ci`.
- pi.dev operator shell (optional) layered on the nix devshell:
  `pi-coding-agent` in the devshell, tracked `.pi/settings.json`
  pinning `dev-loops@0.9.0`, repo policy in `.devloops`, docs in
  `doc/pi-development.md`.
- Release pipeline (`.github/workflows/release.yml`): tag-triggered
  verify → crates.io publish (dependency-ordered `PUBLISH_CRATES`,
  currently `midnight-did-domain` only) → GitHub Release; publishing
  policy in `doc/publishing.md`.
- Documentation: README rewritten around the repo mission (Rust
  libraries for the Midnight SSI domain) with crate/publishability
  table and branch model; `midnight-did-runtime` crate README; research
  docs for VC shared components and the Rust DID resolver
  (`doc/research/`).

### Changed

- CI: `DeterminateSystems/*` actions pinned to release SHAs (previously
  `@main`); `persist-credentials: false` on all checkouts;
  `timeout-minutes` on every job; push triggers repointed from `main`
  to `rust-codegen` + `develop`; `codegen-check` now runs on every
  PR/push (the `midnightntwrk/midnight-did` submodule went public, so
  the default token fetches it).

## [0.5.0] — 2026-08-05

### Overview

`0.5.0` migrates the workspace onto the **did.compact 0.5.0** contract —
the controller-authorization + recovery redesign — and closes the codegen
gaps that porting it required. It also wires the new
`recoverControllerKey` operation end-to-end. The wallet+proof-server+indexer
bridge that turns `LiveBackend` from `todo!()` stubs into production paths
is the remaining follow-up tracked under this line (see below).

### Contract 0.5.0 migration (did.compact controller-authorization + recovery)

Re-generated `midnight-did-runtime`'s `contract/generated.rs` from the
**midnight-did 0.5.0** contract (`third_party/midnight-did` submodule
pinned to `midnightntwrk/midnight-did` `main` @ `42a8e4a`, "update docs
and package baseline to 0.5.0"). This is the controller-authorization +
recovery redesign: JubjubPoint `controllerPublicKey` /
`recoveryAuthorityPublicKey` ledger fields, `localControllerPublicKey` /
`localRecoveryAuthorityPublicKey` witnesses, Schnorr-signature-authorized
mutations, `MapMutation` / `SetMutation` mutation enums, a
`recoverControllerKey` circuit, and seven `CurveType` variants (adds
`BLS12381G1` / `BLS12381G2`).

- `flake.nix` / `flake.lock`: `compact` input repointed to the
  `yshyn-iohk/compact` `did-0.5.0-codegen` branch, whose `compactc`
  gained the codegen support the 0.5.0 contract needs — a JubjubPoint
  ledger-read decoder + typed initial-cell default, interleaved
  bare-call-in-if-branch mutation bodies, ctx-arg hoisting for impure
  calls, constructor-mode impure-circuit context threading, and
  multi-assert if/else branches. The regenerated contract compiles
  cleanly (`cargo build -p midnight-did-runtime`) and `just codegen`
  is idempotent.
- `third_party/midnight-did`: submodule pin corrected to `42a8e4a`
  (upstream `main`); the previous pin (`6274cff`) was a divergent
  local-mirror line ("Redesign DID verification method storage",
  `Bytes<32>` controller keys) that is not on upstream `main`.
- Wired the new `recoverControllerKey` operation (recovery-authority-
  authorized controller-key reset) end-to-end, mirroring
  `rotateControllerKey`: `DidContractCall::RecoverControllerKey`,
  `Contract::recover_controller_key`,
  `controller_operations::recover_controller_key` (+
  `recover_controller_key_with_derivation` /
  `recover_did_controller_key` re-export), the
  `midnight_did_uniffi::recover_controller_key` FFI entry point, and a
  `recover` step in the reference CLI's demo flow — each with round-trip
  tests. Ports `recoverControllerKey` from
  `packages/api/src/controller-operations.ts`; consistent with this
  crate's existing design, the recovery-authority signature + on-ledger
  recovery-authority match are the deploy-backend's responsibility
  (callers pass the already-derived new controller public key).

### Review follow-ups (Codex CLI)

- **FFI controller-key operations now persist the correct secret.** The uniffi
  `rotate_controller_key` / `recover_controller_key` entry points previously
  passed a zero secret to a **per-call** `InMemoryPrivateStateStore` that was
  dropped on return — so neither the right secret nor any state actually
  survived the call. Both now accept `new_secret_key_hex` (breaking FFI
  signature change), and `DidServiceHandle` holds a **shared** private-state
  store threaded into `create_did` / rotation / recovery, so a create → rotate
  → recover sequence keeps the promoted active secret (covered by a new
  `controller_state_persists_across_calls_on_one_handle` test).
- **Constructor codegen threads zswap-local state** through impure-circuit
  calls (compiler-side, regenerated here): the generated constructor now
  carries the callee's returned `current_zswap_local_state` into the
  `ConstructorResult` instead of restarting from empty — see
  yshyn-iohk/compact `did-0.5.0-codegen` (A25). No behavioural change for the
  0.5.0 contract (its constructor's only impure call is a pure-assert), but
  correct for any future zswap-affecting constructor circuit.

### Still outstanding in 0.5.0

The wallet+proof-server+indexer bridge — turning `LiveBackend::submit_tx` /
`read_snapshot` from `todo!()` stubs into production paths — is the remaining
work on this line; see
[doc/adr/0008-contract-abstraction-reform.md](doc/adr/0008-contract-abstraction-reform.md)
("Future work") for the four-step closure plan.

## [0.4.1] — 2026-06-26

### Overview

`0.4.1` is the **builder + decode validation closure** patch. It
locks both sides of `BuiltTx::bytes` for the SchnorrJubjub
ledger-shape types — callers can no longer struct-literal a
malformed value (encoding side), and an incoming envelope decoded
via `RecordingBackend::submit_tx` (or any future `LiveBackend`)
cannot land a malformed inner value either (decoding side). Wire
format remains byte-identical with v0.4.0 for valid inputs.

Surfaces a real correctness finding from the architecture audit
(2026-06-26 `docs/superpowers/notes/2026-06-26-architecture-audit.md`
Rec #1): three at-risk ledger-shape types had public `String` /
`[String; 4]` fields that accepted arbitrary garbage in test
fixtures. The fix closes the bypass on both encode and decode
sides.

### Added

- **Validating `::new` constructors** on `JubjubPointHex`,
  `SchnorrJubjubSignature`, `SchnorrJubjubDigest`, and the
  api-layer `SchnorrJubjubVerificationMethod` wrapper. Each
  rejects malformed hex / wrong byte length / empty id at
  construction time.
- **Decode-side validation gates** via
  `#[serde(try_from = "Repr")]` shims on
  `JubjubPointHex` + `SchnorrJubjubSignature`, plus a hand-rolled
  `Deserialize` on the `#[serde(transparent)]`
  `SchnorrJubjubDigest` (transparent + try_from are mutually
  exclusive — the hand-rolled impl pulls `<[String; 4]>::deserialize`
  then runs `::new`).
- **34 new regression tests** across two files:
  - `crates/midnight-did-api/tests/builder_validation.rs` —
    19 encode-side tests (`"01"` short coord, `"deadbeef"` short
    sig, `"1"` short digest limb, empty id).
  - `crates/midnight-did-api/tests/decode_validation.rs` —
    15 decode-side tests: 10 negative cases (legacy stubs
    decode → reject), 3 positive round-trip
    (encode → decode → re-encode is byte-identical), 2
    envelope-level tests (the gate applies transitively when
    `DidContractCall::decode` walks a malformed JSON payload).

### Changed

- **`JubjubPointHex`, `SchnorrJubjubSignature`, `SchnorrJubjubDigest`
  fields are now private.** Callers must use `::new(NewX)?` (or
  decode through the validating Deserialize path) — struct-literal
  construction of these types is no longer possible.
- **`SchnorrJubjubVerificationMethod`** wrapper gets a fallible
  `::new(NewSchnorrJubjubVerificationMethod) -> Result<Self,
  ApiError>` constructor enforcing non-empty `id`.

### Developer experience

- **Justfile `codegen` recipe path fix** — recipe targeted
  `crates/midnight-did/src/contract/` (pre-4-crate-split, ADR 0003);
  now correctly targets `crates/midnight-did-runtime/src/contract/`
  matching the v0.4.0 crate layout. `just codegen` and
  `just codegen-check` now work end-to-end again.

### References

- ADR 0008 ("Builder + decode validation gate" section):
  [doc/adr/0008-contract-abstraction-reform.md](doc/adr/0008-contract-abstraction-reform.md)
- Architecture audit Rec #1 (the finding):
  `docs/superpowers/notes/2026-06-26-architecture-audit.md`
- Encoding-side commit:
  [`59ed1f5`](https://github.com/MediaNoxLabs/midnight-identity/commit/59ed1f5)
- Decoding-side commits:
  [`b3fdb20`](https://github.com/MediaNoxLabs/midnight-identity/commit/b3fdb20),
  [`3080d49`](https://github.com/MediaNoxLabs/midnight-identity/commit/3080d49),
  [`8d9df0d`](https://github.com/MediaNoxLabs/midnight-identity/commit/8d9df0d)

## [0.4.0] — 2026-06-25

### Overview

`0.4.0` is the **R2 contract-abstraction reform** release. The
12-method `DidContract` async trait + `mock::RecordingContract`
shim retire, replaced by a concrete `Contract<B: Backend>` wrapper
in `midnight-did-runtime` plus a 14-variant `DidContractCall` enum
that flows through `Backend::submit_tx`. Wire format remains
byte-identical with the TypeScript reference; public Rust API
surface has breaking changes per below.

R2-2 + R2-3 ship via the **Path 2** strategy (see
[ADR 0008](doc/adr/0008-contract-abstraction-reform.md)):
`Contract<B>` encodes typed call variants into `BuiltTx::bytes`
instead of delegating to `generated::Contract<PS, W>`. The spec's
original delegation template is gated on the
wallet+proof-server+indexer bridge (`LiveBackend::submit_tx`
remains `todo!()`); Path 2 sidesteps that by keeping the public
Rust API the shape it will be when the bridge lands and routing
test coverage through `RecordingBackend`.

### Added

- **`midnight_did_runtime::Contract<B: Backend>`** — concrete
  wrapper struct (12 inherent async methods, one per exported
  `did.compact` circuit) replacing the trait-erased `&dyn
  DidContract` seam. Each method builds a typed `DidContractCall`
  variant, encodes via `bincode`, and submits through the backend.
- **`midnight_did_runtime::DidContractCall`** — 14-variant tagged
  enum with `bincode` `encode`/`decode` for transport via
  `BuiltTx::bytes`. Payload shapes mirror the v0.3.0
  `RecordedCall::X` variants 1:1 so test migration is mechanical.
- **`Backend::read_snapshot(&self) -> Result<DidLedgerSnapshot,
  BackendError>`** — third trait method exposing the high-level
  api-shape snapshot. `LiveBackend::read_snapshot` is `todo!()`
  until the `Ledger → DidLedgerSnapshot` adapter lands.
  `RecordingBackend::with_snapshot(...)` /
  `ResolverBackend::new(snapshot, ...)` return the configured
  snapshot.
- **`RecordingBackend::recorded_calls(&self) -> Vec<DidContractCall>`**
  — accessor used by the migrated integration tests instead of the
  deleted `RecordingContract::calls()`.

### Changed

- **All 5 operation-builder modules**
  (`did_operations`, `controller_operations`,
  `verification_method_operations`, `service_operations`,
  `document_operations`) now take `&Contract<B: Backend>` directly.
  Each previously `<C: DidContract + ?Sized>(contract: &C, ...)`
  signature is now `<B: Backend>(contract: &Contract<B>, ...)`.
- **56 integration tests across 4 files** migrated 1:1:
  `RecordingContract::new(ADDR, NET)` →
  `Contract::new(RecordingBackend::with_snapshot(snapshot), ADDR,
  NET)`, `RecordedCall::X(payload)` →
  `DidContractCall::X { payload fields }`.
- **`midnight-did-api` depends on `midnight-did-runtime`.** The api
  crate previously held the contract-abstraction shape
  (`DidContract` trait); that surface is now in the runtime crate
  where it belongs.

### Removed

- **`midnight_did_api::contract::DidContract`** async trait
  (12 methods). Migrate consumers to `Contract<B: Backend>` from
  `midnight-did-runtime`.
- **`midnight_did_api::contract::mock::RecordingContract`** mock.
  Migrate test setups to
  `Contract::new(RecordingBackend::with_snapshot(...), ...)`.
- **`midnight_did_api::contract::mock::RecordedCall`** enum.
  Variants are preserved 1:1 as
  `midnight_did_runtime::DidContractCall::X { ... }`; matchers
  switch from `RecordedCall::X(payload)` tuple-pattern to
  `DidContractCall::X { fields, .. }` struct-pattern.

### References

- ADR 0008 — contract-abstraction reform (Path 2 rationale + future
  work):
  [doc/adr/0008-contract-abstraction-reform.md](doc/adr/0008-contract-abstraction-reform.md)
- R2 design spec:
  [doc/specs/2026-06-24-r2-contract-abstraction-design.md](doc/specs/2026-06-24-r2-contract-abstraction-design.md)
- Fully supersedes ADR 0002 (trait-erasure-for-contract):
  [doc/adr/0002-trait-erasure-for-contract.md](doc/adr/0002-trait-erasure-for-contract.md)

## [0.3.0] — 2026-06-25

### Overview

`0.3.0` is the **R1 finish + R2-1 scaffold** release. It closes the
two `0.2.0`-deferred steps (4b/4c) and lands R2-1 (Backend trait
scaffold) while explicitly deferring R2-2/R2-3 to a future release
(see ADR 0008). Wire format remains byte-identical with the
TypeScript reference. Public Rust API surface has breaking changes
per below.

The cycle's codegen-side milestone — `did.compact` compiles
end-to-end through `compactc --rust` → `cargo check -p
midnight-did-runtime` clean — also lands here via the bumped compact
pin (`960fc26`) and the regenerated `contract/generated.rs`.

### Added

- **`midnight_did_runtime::backend`** — `Backend` async trait
  (`submit_tx`, `read_state`) plus `LiveBackend` (stub — `todo!()`
  until the wallet+proof+indexer bridge lands), `RecordingBackend`
  (Mutex-guarded tx + state snapshot for tests), `ResolverBackend`
  (read-only, rejects `submit_tx`). Forward-compatible scaffold for
  the R2 contract abstraction reform — not yet a callable surface;
  consumers continue to use `DidContract` (`midnight-did-api`) until
  R2-2 lands. See ADR 0008.
- **Accessor methods** on the now-private struct fields:
  `PublicKeyJwk::{kty, crv, x, y, extensions}`,
  `VerificationMethod::{id, type_, controller, public_key_jwk}`,
  `Service::{id, type_, service_endpoint}`, plus `as_str` /
  `into_string` on `DidString` / `DidUrl` / `RelativeUrl`.

### Changed

- **R1 step 4b — privatized fields**: `VerificationMethod`,
  `Service`, `PublicKeyJwk`, `DidString`, `DidUrl`, `RelativeUrl`
  inner fields are now private. The only way to construct these
  values is `::new(NewX) -> Result<Self, ValidationError>`; the only
  way to read them is via accessor methods. Closes the
  "callers can bypass `::new` by struct-literal construction" hole
  `0.2.0` left.
- **R1 step 4c — call-site migration**: the remaining ~17 direct
  struct-literal construction sites across `midnight-did-api`,
  `midnight-did-method`, `midnight-did-cli`, and the integration
  tests are migrated to `::new(NewX)?`. The 4 negative test
  fixtures that previously asserted on
  `verification_method_to_ledger`'s rejection path now assert on
  `PublicKeyJwk::new`'s error path directly (the rejection moved
  upstream when `::new` became fallible).
- **`compact_runtime` pin bumped** (`flake.lock` →
  `yshyn-iohk/compact@960fc26`) — picks up the codegen-rust
  branch's A18/A19, Bug-1..7, R5a/R5b, and Module-1 closures.
  `crates/midnight-did-runtime/src/contract/generated.rs` regen
  output now compiles `cargo check`-clean for the full
  `did.compact` source.

### Removed

- **R1 step 4c — `create_*` helpers retired**:
  `create_verification_method(CreateVerificationMethodParams)`,
  `create_service(CreateServiceParams)`,
  `CreateVerificationMethodParams`, and `CreateServiceParams` are
  gone, along with their `pub use` re-exports from
  `midnight_did_domain::lib`. External callers should switch
  `create_verification_method(p)` →
  `VerificationMethod::new(NewVerificationMethod { ... })?`.

### References

- ADR 0008 — R2-2/R2-3 deferred:
  [doc/adr/0008-contract-abstraction-reform-deferred.md](doc/adr/0008-contract-abstraction-reform-deferred.md)
- ADR 0005 updated with A1–A19 walker-gap closure log:
  [doc/adr/0005-codegen-gap-handling.md](doc/adr/0005-codegen-gap-handling.md)
- Compact codegen-rust branch (cycle's headline milestone):
  https://github.com/yshyn-iohk/compact/tree/codegen-rust

## [0.2.0] — 2026-06-23

### Overview

`0.2.0` is the **type-safety sweep** release: the first
architectural reset after the initial TS port. The audit
(`doc/specs/2026-06-23-r1-type-safety-sweep-design.md`) identified
five high-impact TS-isms that didn't pay rent in Rust; this
release closes six of the eight remediation steps, with the
remaining two (steps 4b/4c — private fields + mass call-site
migration) deferred to `0.3.0`.

Wire format (JSON byte-parity with the TypeScript `@midnight-ntwrk/midnight-did-*`
reference) is **unchanged**. In-memory representations and the
public Rust API surface have breaking changes per below.

### Added

- **`midnight_did_method::hex_ext`** — `HashOutputExt` trait
  providing `from_hex(&str)` / `to_hex()` round-trips for the
  upstream `midnight_base_crypto::hash::HashOutput` and
  `compact_runtime::ContractAddress` types. Upstream `Display`
  is intentionally truncated to 10 hex chars for logs; this
  trait covers the full 64-char round-trip the DID document
  wire format uses.
- **`midnight_did_domain::ids`** — new module containing the
  W3C-DID identifier newtypes `DidKeyId`, `FragmentId`,
  `ServiceId`. Each has a private inner field, validating
  `Self::new(impl Into<String>) -> Result<Self, IdError>`,
  validating `Deserialize` impl that delegates to `::new`, and
  `#[serde(transparent)]` `Serialize` keeping the wire format
  identical.
- **`VerificationMethod::new(NewVerificationMethod)`**,
  **`Service::new(NewService)`**,
  **`PublicKeyJwk::new(NewPublicKeyJwk)`** — fallible inherent
  constructors that return `Result<Self, ValidationError>`. The
  pre-existing `create_verification_method` /
  `create_service` factories now delegate to these.
- **`PublicKeyJwkWire`** — wire-format shim used by
  `PublicKeyJwk`'s validating `Deserialize` via `#[serde(try_from
  = "PublicKeyJwkWire")]`. Invalid JWKs (OKP with `y`, RSA
  without `y`, private-key material in extensions, ...) are
  now rejected at the serde gate, not silently accepted.
- **`DidDocumentBuilder`** — fluent builder for `DidDocument`
  with cross-reference validation on `build()`: subject DID
  parse, no duplicate verification-method ids, no duplicate
  service ids, every relation entry references an existing
  verification-method.
- **`VerificationError`**, **`ControllerError`** — new domain
  error enums. `ContractError` (already existed) joins them as
  the third domain-grouped enum.

### Changed

- **Upstream primitives reused directly.** The local
  `pub struct ContractAddress(pub String)` and
  `pub struct OffchainStateHashHex(pub String)` shadow newtypes
  are gone. Both are now re-exported from the upstream Midnight
  ledger libraries:
  - `ContractAddress` is `midnight_coin_structure::contract::ContractAddress(pub HashOutput)`
    (re-exported via `compact_runtime::ContractAddress`).
  - `OffchainStateHashHex` is `midnight_base_crypto::hash::HashOutput`.
  In-memory shape is now `[u8; 32]` instead of a 64-char String;
  all upstream derives come along (`FieldRepr` / `FromFieldRepr` /
  `BinaryHashRepr` / `Serializable` / serde / `Zeroize` /
  constant-time eq).
- **`MidnightSubjectId::as_hex(&self) -> &str` →
  `MidnightSubjectId::to_hex(&self) -> String`.** The
  pre-v0.2.0 borrowable `&str` form is impossible because
  storage is now bytes; hex rendering is on demand.
- **`parse_contract_address` returns the upstream
  `ContractAddress` type** via `HashOutputExt::from_hex`.
  Mixed-case input is normalised to lowercase internally.
- **`parse_offchain_state_hash` returns `HashOutput`** via the
  same trait; lowercase invariant unchanged.
- **`create_did(.., secret_key: [u8; 32])`** — the parameter is
  now required. The pre-v0.2.0 `Option<[u8; 32]>` shape silently
  fell back to `[0u8; 32]` when `None` was passed; that footgun
  is gone. The library never decides whether to generate or
  accept key material; callers supply explicit bytes.
- **`ApiError` is now an umbrella with domain-grouped lifts.**
  The flat 13-variant enum is split:
  - `ApiError::Verification(VerificationError)` — relation
    add/remove failures.
  - `ApiError::Controller(ControllerError)` — rotation
    orphaned, invalid secret length, controller/subject
    mismatch.
  - `ApiError::Contract(ContractError)` — on-chain call failures.

  Each domain enum lifts into `ApiError` via `#[from]`, so the
  `?` operator continues to work transparently.

### Removed

- `pub struct ContractAddress(pub String)` shadow newtype.
  Migrate by switching to the upstream type:
  `use compact_runtime::ContractAddress;`. Replace any String
  field access with `addr.to_hex()` (via `HashOutputExt`).
- `pub struct OffchainStateHashHex(pub String)` shadow newtype.
  Same migration via `midnight_base_crypto::hash::HashOutput`.
- `parse_contract_address`, `parse_offchain_state_hash` —
  superseded by `HashOutputExt::from_hex`. The wrappers still
  exist with their original names for now, but return the
  upstream type instead of the String shadow.
- `MidnightSubjectId::as_hex(&self) -> &str` — superseded by
  `to_hex(&self) -> String`.
- `ApiError::ControllerRotationOrphaned`,
  `ApiError::RelationAlreadyContains`,
  `ApiError::RelationMissing`, `ApiError::InvalidSecretKey`,
  `ApiError::ControllerSubjectMismatch` — moved into
  domain-grouped enums (see Changed).

### Deferred to 0.3.0

The two remaining steps from the R1 spec:

- **Step 4b** — make `VerificationMethod` / `Service` /
  `PublicKeyJwk` fields private; add accessor methods. Without
  this, callers can still bypass `::new` by constructing via
  `Foo { id: ..., ... }`. The new `::new` constructors are the
  *recommended* path in 0.2.0; 0.3.0 will make them the *only*
  path.
- **Step 4c** — migrate the ~114 existing direct
  struct-literal construction sites to `::new(NewX)` and retire
  the `create_verification_method` / `create_service` free
  functions. Mechanical mass-replace work that benefits from a
  dedicated session.

### Test coverage

Workspace test count: **144 (pre-R1) → 231 (post-R1)** = +87 new
tests across 4 new integration test files
(`tests/hex_ext.rs`, `tests/ids.rs`, `tests/constructors.rs`,
`tests/did_document_builder.rs`, `tests/error_hierarchy.rs`).
All pre-R1 tests still pass.

### References

- Design spec: [doc/specs/2026-06-23-r1-type-safety-sweep-design.md](doc/specs/2026-06-23-r1-type-safety-sweep-design.md)
- Architecture decision record: [doc/adr/0007-type-safety-sweep.md](doc/adr/0007-type-safety-sweep.md)
- Supersedes (partial): [doc/adr/0002-trait-erasure-for-contract.md](doc/adr/0002-trait-erasure-for-contract.md),
  [doc/adr/0004-private-state-as-trait.md](doc/adr/0004-private-state-as-trait.md)

## [0.1.0] — 2026-06-03

Initial release: TypeScript-port baseline of the Midnight DID
Rust crates (`midnight-did-domain`, `-method`, `-api`, `-cli`,
`-uniffi`, `-runtime`) plus the umbrella `midnight-did` crate.
See `doc/architecture.md` and ADRs 0001–0006.
