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

//! Credential-model descriptors — port of `packages/core/model/src/types.ts`.
//!
//! Every struct uses `#[serde(rename_all = "camelCase")]` and every enum
//! `#[serde(rename_all = "kebab-case")]` so the JSON wire form is byte-identical
//! to the TypeScript package's. Optional fields are `Option<_>` with
//! `skip_serializing_if` so an absent TS property stays absent, rather than
//! becoming `null`.
//!
//! ## Deliberate deviations from the TypeScript source
//!
//! - `CredentialCodec` / `PresentationCodec` are TS *interfaces carrying
//!   methods*, so they become Rust **traits** ([`CredentialCodec`],
//!   [`PresentationCodec`]). Their only data member — `mediaType` — is
//!   modelled separately as [`CodecDescriptor`], which is what
//!   [`CredentialFamilyDefinition`] stores. This keeps the family definition
//!   pure data (serializable, `PartialEq`) while Rust's type system statically
//!   guarantees the `encode`/`decode` members the TS validator has to check at
//!   runtime.
//! - `CredentialFamilyDefinition` is therefore **not generic**: the TS type
//!   parameters exist solely to type the two codecs.
//! - TS tuple types `readonly [string, ...string[]]` (non-empty arrays) become
//!   `Vec<String>`; the non-emptiness invariant is enforced by
//!   [`crate::validation`], exactly as the TS validator enforces it at runtime.

use serde::{Deserialize, Serialize};

/// How a claim is revealed when the credential is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClaimDisclosure {
    /// Present in the clear in the credential body.
    Public,
    /// Revealable per-presentation at the holder's discretion.
    Selective,
    /// Only a commitment appears on-chain; the value stays with the holder.
    Committed,
    /// Never revealed — only predicates over the value are provable.
    PredicateOnly,
}

/// Category of an optional/required capability a credential family composes in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CredentialCapabilityKind {
    /// Binds the credential to a holder (DID, Jubjub key, secret, …).
    HolderBinding,
    /// Revocation / suspension status.
    Status,
    /// A ZK proof capability.
    Proof,
    /// A presentation-side capability.
    Presentation,
}

/// Where a single claim lives in the credential body, and how it is disclosed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialClaimDescriptor {
    /// Stable identifier, unique within the schema's claim list.
    pub id: String,
    /// Non-empty path of body segments locating the claim.
    pub path: Vec<String>,
    /// Disclosure treatment for this claim.
    pub disclosure: ClaimDisclosure,
    /// Whether the claim must be present in a conforming credential.
    pub required: bool,
    /// Optional value-type hint (free-form identifier).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
}

/// The claim shape of a credential family, versioned independently of the family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialSchemaDescriptor {
    /// Stable schema identifier.
    pub id: String,
    /// Semantic version of the schema.
    pub version: String,
    /// Non-empty list of credential type names this schema realises.
    pub credential_types: Vec<String>,
    /// Claim descriptors; `id`s must be unique.
    pub claims: Vec<CredentialClaimDescriptor>,
}

/// A capability the family composes in, e.g. a holder binding or a status mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCapabilityDescriptor {
    /// Stable capability identifier, unique within the family.
    pub id: String,
    /// Capability category.
    pub kind: CredentialCapabilityKind,
    /// Optional semantic version of the capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether verifiers must support this capability.
    pub required: bool,
}

/// Role a ZK artifact plays for a credential family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofArtifactPurpose {
    /// Proving key.
    Prover,
    /// Verifier key.
    Verifier,
    /// Circuit description (e.g. ZKIR).
    Circuit,
    /// Accompanying metadata.
    Metadata,
}

/// One ZK artifact a family needs distributed to provers/verifiers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofArtifactRequirement {
    /// Stable artifact identifier, unique within the family.
    pub id: String,
    /// IANA-style media type of the artifact bytes.
    pub media_type: String,
    /// Role the artifact plays.
    pub purpose: ProofArtifactPurpose,
    /// Whether the artifact may be absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

/// A package a credential family requires at composition time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialPackageRequirement {
    /// npm-style package name (optionally `@scope/`-prefixed).
    pub name: String,
    /// Registry-resolvable exact or bounded semantic version.
    pub version: String,
    /// Explicit package export subpaths that must be available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exports: Option<Vec<String>>,
}

/// The set of packages that together realise a credential family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialCompositionManifest {
    /// Manifest format version; only [`Self::FORMAT_VERSION`] is accepted.
    pub format_version: u32,
    /// Required packages; `name`s must be unique.
    pub packages: Vec<CredentialPackageRequirement>,
}

impl CredentialCompositionManifest {
    /// The only manifest format version the TypeScript source accepts (`1`).
    pub const FORMAT_VERSION: u32 = 1;

    /// Build a manifest at [`Self::FORMAT_VERSION`].
    #[must_use]
    pub fn new(packages: Vec<CredentialPackageRequirement>) -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            packages,
        }
    }
}

/// The data half of a codec: the media type it reads and writes.
///
/// See the module docs for why this is split out of [`CredentialCodec`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecDescriptor {
    /// Media type of the encoded form.
    pub media_type: String,
}

impl CodecDescriptor {
    /// Build a descriptor for `media_type`.
    #[must_use]
    pub fn new(media_type: impl Into<String>) -> Self {
        Self {
            media_type: media_type.into(),
        }
    }
}

/// Encode/decode a credential to and from a transport form.
///
/// Rust counterpart of the TS `CredentialCodec<TCredential, TEncoded>`
/// interface.
pub trait CredentialCodec {
    /// The in-memory credential type.
    type Credential;
    /// The encoded transport type.
    type Encoded;
    /// Error surfaced by [`Self::decode`].
    type Error;

    /// Media type of [`Self::Encoded`].
    fn media_type(&self) -> &str;

    /// Encode a credential.
    fn encode(&self, credential: &Self::Credential) -> Self::Encoded;

    /// Decode a credential.
    fn decode(&self, encoded: &Self::Encoded) -> Result<Self::Credential, Self::Error>;

    /// The codec's data half, for embedding in a [`CredentialFamilyDefinition`].
    fn descriptor(&self) -> CodecDescriptor {
        CodecDescriptor::new(self.media_type())
    }
}

/// Encode/decode a presentation to and from a transport form.
///
/// Rust counterpart of the TS `PresentationCodec<TPresentation, TEncoded>`
/// interface.
pub trait PresentationCodec {
    /// The in-memory presentation type.
    type Presentation;
    /// The encoded transport type.
    type Encoded;
    /// Error surfaced by [`Self::decode`].
    type Error;

    /// Media type of [`Self::Encoded`].
    fn media_type(&self) -> &str;

    /// Encode a presentation.
    fn encode(&self, presentation: &Self::Presentation) -> Self::Encoded;

    /// Decode a presentation.
    fn decode(&self, encoded: &Self::Encoded) -> Result<Self::Presentation, Self::Error>;

    /// The codec's data half, for embedding in a [`CredentialFamilyDefinition`].
    fn descriptor(&self) -> CodecDescriptor {
        CodecDescriptor::new(self.media_type())
    }
}

/// A complete, protocol-neutral description of a credential family.
///
/// Validate with [`CredentialFamilyDefinition::validate`] or construct through
/// [`crate::validation::define_credential_family`] (the counterpart of the TS
/// `defineCredentialFamily`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFamilyDefinition {
    /// Stable family identifier.
    pub id: String,
    /// Semantic version of the family.
    pub version: String,
    /// Claim shape.
    pub schema: CredentialSchemaDescriptor,
    /// Composed capabilities; `id`s must be unique.
    pub capabilities: Vec<CredentialCapabilityDescriptor>,
    /// Required ZK artifacts; `id`s must be unique.
    pub artifacts: Vec<ProofArtifactRequirement>,
    /// Packages realising the family.
    pub composition: CredentialCompositionManifest,
    /// Media type of the credential codec.
    pub credential_codec: CodecDescriptor,
    /// Media type of the presentation codec.
    pub presentation_codec: CodecDescriptor,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize `value`, assert the JSON matches `expected` exactly, then
    /// deserialize it back and assert structural equality.
    fn assert_wire_round_trip<T>(value: &T, expected: &str)
    where
        T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).expect("serialize");
        assert_eq!(json, expected, "wire spelling drifted from the TypeScript form");
        let round_tripped: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&round_tripped, value);
    }

    #[test]
    fn claim_disclosure_wire_spellings() {
        for (variant, literal) in [
            (ClaimDisclosure::Public, "\"public\""),
            (ClaimDisclosure::Selective, "\"selective\""),
            (ClaimDisclosure::Committed, "\"committed\""),
            (ClaimDisclosure::PredicateOnly, "\"predicate-only\""),
        ] {
            assert_wire_round_trip(&variant, literal);
        }
    }

    #[test]
    fn capability_kind_wire_spellings() {
        for (variant, literal) in [
            (CredentialCapabilityKind::HolderBinding, "\"holder-binding\""),
            (CredentialCapabilityKind::Status, "\"status\""),
            (CredentialCapabilityKind::Proof, "\"proof\""),
            (CredentialCapabilityKind::Presentation, "\"presentation\""),
        ] {
            assert_wire_round_trip(&variant, literal);
        }
    }

    #[test]
    fn artifact_purpose_wire_spellings() {
        for (variant, literal) in [
            (ProofArtifactPurpose::Prover, "\"prover\""),
            (ProofArtifactPurpose::Verifier, "\"verifier\""),
            (ProofArtifactPurpose::Circuit, "\"circuit\""),
            (ProofArtifactPurpose::Metadata, "\"metadata\""),
        ] {
            assert_wire_round_trip(&variant, literal);
        }
    }

    #[test]
    fn claim_descriptor_omits_absent_value_type() {
        let claim = CredentialClaimDescriptor {
            id: "dateOfBirth".into(),
            path: vec!["credentialSubject".into(), "dateOfBirth".into()],
            disclosure: ClaimDisclosure::PredicateOnly,
            required: true,
            value_type: None,
        };
        assert_wire_round_trip(
            &claim,
            r#"{"id":"dateOfBirth","path":["credentialSubject","dateOfBirth"],"disclosure":"predicate-only","required":true}"#,
        );
    }

    #[test]
    fn claim_descriptor_emits_present_value_type() {
        let claim = CredentialClaimDescriptor {
            id: "dateOfBirth".into(),
            path: vec!["dateOfBirth".into()],
            disclosure: ClaimDisclosure::Committed,
            required: false,
            value_type: Some("date".into()),
        };
        assert_wire_round_trip(
            &claim,
            r#"{"id":"dateOfBirth","path":["dateOfBirth"],"disclosure":"committed","required":false,"valueType":"date"}"#,
        );
    }

    #[test]
    fn schema_descriptor_uses_camel_case_credential_types() {
        let schema = CredentialSchemaDescriptor {
            id: "birth".into(),
            version: "1.0.0".into(),
            credential_types: vec!["BirthCredential".into()],
            claims: Vec::new(),
        };
        assert_wire_round_trip(
            &schema,
            r#"{"id":"birth","version":"1.0.0","credentialTypes":["BirthCredential"],"claims":[]}"#,
        );
    }

    #[test]
    fn capability_descriptor_omits_absent_version() {
        let capability = CredentialCapabilityDescriptor {
            id: "holder".into(),
            kind: CredentialCapabilityKind::HolderBinding,
            version: None,
            required: true,
        };
        assert_wire_round_trip(
            &capability,
            r#"{"id":"holder","kind":"holder-binding","required":true}"#,
        );
    }

    #[test]
    fn artifact_requirement_wire_form() {
        let artifact = ProofArtifactRequirement {
            id: "issue.prover".into(),
            media_type: "application/octet-stream".into(),
            purpose: ProofArtifactPurpose::Prover,
            optional: Some(false),
        };
        assert_wire_round_trip(
            &artifact,
            r#"{"id":"issue.prover","mediaType":"application/octet-stream","purpose":"prover","optional":false}"#,
        );
    }

    #[test]
    fn package_requirement_omits_absent_exports() {
        let requirement = CredentialPackageRequirement {
            name: "@midnight-ntwrk/midnight-did-credentials".into(),
            version: "^0.1.0".into(),
            exports: None,
        };
        assert_wire_round_trip(
            &requirement,
            r#"{"name":"@midnight-ntwrk/midnight-did-credentials","version":"^0.1.0"}"#,
        );
    }

    #[test]
    fn composition_manifest_new_pins_format_version() {
        let manifest = CredentialCompositionManifest::new(vec![CredentialPackageRequirement {
            name: "pkg".into(),
            version: "1.2.3".into(),
            exports: Some(vec![".".into()]),
        }]);
        assert_eq!(manifest.format_version, CredentialCompositionManifest::FORMAT_VERSION);
        assert_wire_round_trip(
            &manifest,
            r#"{"formatVersion":1,"packages":[{"name":"pkg","version":"1.2.3","exports":["."]}]}"#,
        );
    }

    #[test]
    fn codec_descriptor_wire_form() {
        assert_wire_round_trip(
            &CodecDescriptor::new("application/json"),
            r#"{"mediaType":"application/json"}"#,
        );
    }

    struct JsonCredentialCodec;

    impl CredentialCodec for JsonCredentialCodec {
        type Credential = String;
        type Encoded = String;
        type Error = std::convert::Infallible;

        fn media_type(&self) -> &str {
            "application/vc+json"
        }

        fn encode(&self, credential: &Self::Credential) -> Self::Encoded {
            format!("vc:{credential}")
        }

        fn decode(&self, encoded: &Self::Encoded) -> Result<Self::Credential, Self::Error> {
            Ok(encoded.trim_start_matches("vc:").to_owned())
        }
    }

    struct JsonPresentationCodec;

    impl PresentationCodec for JsonPresentationCodec {
        type Presentation = String;
        type Encoded = String;
        type Error = std::convert::Infallible;

        fn media_type(&self) -> &str {
            "application/vp+json"
        }

        fn encode(&self, presentation: &Self::Presentation) -> Self::Encoded {
            format!("vp:{presentation}")
        }

        fn decode(&self, encoded: &Self::Encoded) -> Result<Self::Presentation, Self::Error> {
            Ok(encoded.trim_start_matches("vp:").to_owned())
        }
    }

    #[test]
    fn credential_codec_trait_round_trips_and_derives_its_descriptor() {
        let codec = JsonCredentialCodec;
        let encoded = codec.encode(&"body".to_owned());
        assert_eq!(encoded, "vc:body");
        assert_eq!(codec.decode(&encoded).expect("decode"), "body");
        assert_eq!(codec.descriptor(), CodecDescriptor::new("application/vc+json"));
    }

    #[test]
    fn presentation_codec_trait_round_trips_and_derives_its_descriptor() {
        let codec = JsonPresentationCodec;
        let encoded = codec.encode(&"body".to_owned());
        assert_eq!(encoded, "vp:body");
        assert_eq!(codec.decode(&encoded).expect("decode"), "body");
        assert_eq!(codec.descriptor(), CodecDescriptor::new("application/vp+json"));
    }
}
