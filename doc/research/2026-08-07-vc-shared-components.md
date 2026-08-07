<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Shared components for Midnight Verifiable Credentials support

**Date:** 2026-08-07 · **Status:** research → proposal
**Source repo analyzed:** [midnightntwrk/midnight-verifiable-credentials](https://github.com/midnightntwrk/midnight-verifiable-credentials)
(active pnpm/turbo monorepo, "Compact-first Verifiable Credentials and
Verifiable Presentations for Midnight")

## 1. What the VC stack actually is

Key findings from the TS/Compact reference (paths cited from that repo):

- **Compact-first, not W3C-VC-serialization-first.** The normative spec
  (`docs/spec/midnight-credentials.md`) makes JSON/OpenID *informative
  adapters*; the canonical credential is a Compact
  `Credential<TPublicClaims, TClaimCommitments, THolderBinding, TStatusBinding>`
  whose body root is a `persistentHash`. Explicit non-goals: JSON-LD
  canonicalization, JWT/SD-JWT as canonical signing input. **Do not
  budget Rust crates for JSON-LD / multibase / VC-DM canonicalization.**
- **Single signature suite: Schnorr over Jubjub**, verified in-circuit
  (`proofs.compact`: `verifySignature(pk: JubjubPoint, sig {r, s}, challenge)`),
  challenges domain-separated (`"midnight:vc:issuance"`,
  `"midnight:vc:presentation"`, `"midnight:vc:status-attestation"`).
  Off-chain signing comes from the DID cohort package
  `@midnight-ntwrk/midnight-did-jubjub-schnorr@0.5.0`.
- **Revocation = on-chain revoked-set** (`MerkleTree<32, Bytes<32>>` in
  `revocation-registry.compact`) with non-membership proofs as the
  target; **not** StatusList2021.
- **VC depends on DID exactly through the published DID 0.5.0 cohort**
  (root pnpm overrides pin `midnight-did`, `-api`, `-contract`,
  `-domain`, `-jubjub-schnorr`): holder bindings resolve long-form
  offchain DIDs, `VerificationMethodRef{didContractAddress, methodId}`
  references the on-chain DID contract, and integration provisioning
  drives `createDID` / `addSchnorrJubjubVerificationMethod` /
  `addVerificationMethodRelation`.

## 2. Shared-component candidates (consumed by both DID and VC layers)

| # | Component | Evidence in TS stack | Rust home (proposal) |
|---|---|---|---|
| 1 | **Jubjub-Schnorr signature suite** (sign off-chain, key types, challenge derivation) | `midnight-did-jubjub-schnorr` pkg; in-circuit mirror in `proofs.compact` | New crate `midnight-jubjub-schnorr` (or `midnight-signature-suites`) in this workspace |
| 2 | **JWK ↔ curve-point codecs** (base64url x/y ↔ 32-byte coords, x-only OKP sentinel, `KeyType`/`CurveType`) | `midnight-did-domain` exports; VC **re-implements** base64url/bigint codecs in both DID adapters — duplication to collapse | Already ours: `midnight-did-domain::crypto_codecs` — promote/document as the canonical codec surface |
| 3 | **Verification-method addressing** (`VerificationMethodRef`, domain-separated method-id hashing e.g. `"midnight:offchain:holder-method-id:v1"`) | `types.compact` + `hashOffchainDIDMethodId` in both VC adapters | Small shared module in `midnight-did-method` |
| 4 | **Long-form offchain DID create/resolve** | `createLongFormOffchainMidnightDIDString` / `resolveLongFormOffchainMidnightDID` consumed by VC holder-binding adapters | Already ours: `midnight-did-method::offchain` (note: lowest-coverage module at 47.7% — harden before VC consumes it) |
| 5 | **Ledger→document projection + JWK normalization invariants** | DID resolver's `LedgerToDomain`, rules restated in VC `docs/guides/did-integration-modes.md` | Already ours: api/domain mapping layer |
| 6 | **Domain-separated hashing conventions** (persistent/transient hash tags, NUL-separated SHA-256 domains) | `proofs.compact`, adapters, `did-profile.ts` | Shared `midnight-hash-domains` module/crate so DID + VC tags coexist collision-free |
| 7 | **Contract-call plumbing / providers** (proof-server, indexer public-data, private-state stores) | `midnight-js-*@4.x` providers; `MidnightDIDProviders` | Generalize `midnight-did-runtime`'s `Backend` seam into a DID-agnostic provider crate — same machinery the LiveBackend + resolver work needs |
| 8 | **ZK artifact distribution/discovery** | `credential-proofs` `ProofArtifactRequirement`, ADR-0003 there | Shared artifact-locator abstraction (DID 0.5.0 also ships regenerated ZK artifacts) |
| 9 | **Standalone/integration environment** | `standalone-environment` pkg (docker net, wallet funding, DID profile provisioning) | Shared test-infra crate mirroring our TS-stack integration harness |

## 3. VC-specific components (new crates, not shared)

- `credential-model` equivalent — protocol-neutral schema/claim
  descriptors, codecs, composition manifests (zero-dep pure-domain crate).
- VC/VP Compact core + **generated bindings** — the analogue of
  `midnight-did-runtime` for `vc.compact`/`vp.compact`/`proofs.compact`
  (a second consumer of the compact `--rust` codegen).
- Holder-binding profiles (explicit-DID, Jubjub, offchain-DID, secret,
  blinded-secret) + same-holder capability.
- Status/revocation: registry contract bindings, witness/attestation
  builders, status verification protocol.
- Proof-provider abstraction (`ProofProvider`/`ProofJob`).
- OpenID transport schemas (OID4VCI/OID4VP-shaped JSON envelopes).

## 4. Proposed sequencing

1. **Extract/promote the shared seams already in this repo** (items 2,
   3, 4, 5): document them as VC-consumable public API, harden
   `offchain.rs` coverage.
2. **New crate `midnight-jubjub-schnorr`** (item 1) — port of the DID
   cohort's suite; unblocks both VC issuance signing and DID controller
   flows without the TS dependency.
3. **Provider/backend generalization** (item 7) — shared with the
   LiveBackend + resolver tracks; do once, consume three times.
4. VC core crates (§3) — only after 1–3, pinned to specific upstream
   paths/commits (the VC repo is mid-restructure; package naming is in
   flux).

## 5. Caveats

- The VC repo is actively restructuring (`packages/core/compact` vs
  `packages/core/primitives/credentials` duplication, RFC in
  `docs/architecture/repository-restructure-rfc.md`) — pin analysis and
  ports to commits, as we do for `did.compact` (42a8e4a).
- In-circuit revocation non-membership is not yet landed upstream;
  status-attestation signatures fill the gap. Track before porting.
