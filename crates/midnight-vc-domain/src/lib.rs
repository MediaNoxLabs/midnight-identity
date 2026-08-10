// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Pure-data credential model for Midnight Verifiable Credentials (issue #13,
//! ADR 0009 split rules 1 + 2).
//!
//! This is the VC-layer counterpart of [`midnight-did-domain`]: protocol-neutral
//! schema and claim descriptors, composition manifests, ZK-artifact
//! requirements, codec shapes and the credential-status vocabulary — with **no
//! `midnight-*` dependencies**, so the crate stays wasm-clean and can join the
//! wasm32 CI gate alongside `midnight-did-domain`.
//!
//! [`midnight-did-domain`]: https://docs.rs/midnight-did-domain
//!
//! ## Module layout
//!
//! - [`types`] — credential-family descriptors (`packages/core/model/src/types.ts`).
//! - [`validation`] — the descriptor validators (`.../validation.ts`).
//! - [`errors`] — [`CredentialModelError`] (`.../errors.ts`).
//! - [`status`] — status modes, bindings and policy
//!   (`packages/core/status/src/{bindings,policy}.ts`).
//!
//! ## Upstream provenance
//!
//! The Compact contract sources are vendored as the submodule
//! `third_party/midnight-verifiable-credentials`, pinned to the codegen-surveyed
//! rev `a9f1d451afc10c9c44a2937e880a22870e7b65ed` (2026-06-03). The TypeScript
//! packages ported here (`packages/core/model`, `packages/core/status`) landed
//! upstream **after** that rev, so this port follows `main` rev
//! `b8646e2` — retrievable from the vendored submodule's history with:
//!
//! ```text
//! git -C third_party/midnight-verifiable-credentials show \
//!     b8646e2:packages/core/model/src/types.ts
//! ```
//!
//! The submodule working tree stays on the surveyed rev because the credentials
//! contract changed between the two (`verification-v1.compact`, +471 lines) and
//! only the surveyed rev is known to compile TODO-free. See `CHANGELOG.md`.
//!
//! ## Reuse, not duplication
//!
//! Per `doc/research/2026-08-07-vc-shared-components.md` §2 this crate does not
//! re-implement anything the DID layer already ships: base64url / JWK
//! coordinate codecs live in `midnight_did_domain::crypto_codecs`,
//! method-id hashing and long-form offchain DIDs in
//! `midnight_did_method::holder_binding` / `::offchain`, and Schnorr-over-Jubjub
//! signing in `midnight-did-jubjub-schnorr`.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

/// Crate version reported by the build.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod errors;
pub mod status;
pub mod types;
pub mod validation;

// Re-exports mirroring the upstream `index.ts` barrels so downstream callers can
// `use midnight_vc_domain::*` for the common items.
pub use errors::{CredentialModelError, CredentialModelErrorCode};
pub use status::{
    EnabledStatusMode, FreshnessPolicy, StatusBinding, StatusBindingDetails, StatusBindingForQuery,
    StatusCapabilityDescriptor, StatusCapabilityKind, StatusEvidence, StatusHandle, StatusMode, StatusOpaque,
    StatusPolicy, StatusQuery, StatusReference, StatusState,
};
pub use types::{
    ClaimDisclosure, CodecDescriptor, CredentialCapabilityDescriptor, CredentialCapabilityKind,
    CredentialClaimDescriptor, CredentialCodec, CredentialCompositionManifest, CredentialFamilyDefinition,
    CredentialPackageRequirement, CredentialSchemaDescriptor, PresentationCodec, ProofArtifactPurpose,
    ProofArtifactRequirement,
};
pub use validation::{
    assert_credential_composition_manifest, assert_credential_family_definition, define_credential_family,
    is_package_name, is_package_version, is_semantic_version,
};
