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

//! Credential-model validators — port of `packages/core/model/src/validation.ts`.
//!
//! ## Deliberate deviations from the TypeScript source
//!
//! - The TS validators exist partly to police values Rust's type system already
//!   guarantees (`typeof x === "string"`, `Array.isArray(...)`,
//!   `typeof codec.encode === "function"`, "must be an object"). Those branches
//!   have no Rust counterpart and are omitted; every branch that can actually
//!   fail on well-typed input is preserved with the same error code, path and
//!   message.
//! - The three TS regular expressions are hand-rolled as byte scanners
//!   ([`is_semantic_version`], [`is_package_version`], [`is_package_name`])
//!   rather than pulling in `regex` — the crate stays dependency-light and
//!   wasm-clean. Each scanner is unit-tested against the pattern's accept and
//!   reject classes.
//! - TS's separate `/^(?:file|link|workspace|git|https?):/` guard on package
//!   versions is folded into [`is_package_version`]: those specifiers fail the
//!   version pattern anyway, and the resulting error is byte-identical
//!   (`INVALID_PACKAGE_REQUIREMENT` at `…​.version`).

use std::collections::HashSet;

use crate::errors::{CredentialModelError, CredentialModelErrorCode};
use crate::types::{
    CredentialCompositionManifest, CredentialFamilyDefinition, CredentialPackageRequirement, CredentialSchemaDescriptor,
};

// ---------------------------------------------------------------------------
// Pattern scanners (hand-rolled equivalents of the TS regular expressions)
// ---------------------------------------------------------------------------

/// `0 | [1-9]\d*` — a semantic-version numeric component (no leading zeros).
fn is_numeric_component(value: &str) -> bool {
    match value.as_bytes() {
        [] => false,
        [b'0'] => true,
        [b'0', ..] => false,
        bytes => bytes.iter().all(u8::is_ascii_digit),
    }
}

/// `major.minor.patch`, each component matching [`is_numeric_component`].
fn is_version_core(value: &str) -> bool {
    let mut components = value.split('.');
    let (Some(major), Some(minor), Some(patch), None) = (
        components.next(),
        components.next(),
        components.next(),
        components.next(),
    ) else {
        return false;
    };
    is_numeric_component(major) && is_numeric_component(minor) && is_numeric_component(patch)
}

/// `[0-9A-Za-z.-]+` — the prerelease / build-metadata character class.
fn is_dot_separated_alphanumeric(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

/// TS `semanticVersionPattern`: `core(-prerelease)?(+build)?`.
#[must_use]
pub fn is_semantic_version(value: &str) -> bool {
    // The version core cannot contain '-' or '+', so splitting on the first
    // occurrence of each reproduces the regex's grouping exactly.
    let (without_build, build) = match value.split_once('+') {
        Some((head, build)) => (head, Some(build)),
        None => (value, None),
    };
    if build.is_some_and(|build| !is_dot_separated_alphanumeric(build)) {
        return false;
    }
    let (core, prerelease) = match without_build.split_once('-') {
        Some((head, prerelease)) => (head, Some(prerelease)),
        None => (without_build, None),
    };
    if prerelease.is_some_and(|prerelease| !is_dot_separated_alphanumeric(prerelease)) {
        return false;
    }
    is_version_core(core)
}

/// TS `packageVersionPattern`: an optional range operator, a version core and an
/// optional prerelease — but **no** build metadata.
#[must_use]
pub fn is_package_version(value: &str) -> bool {
    let rest = ["~", "^", ">=", "<=", ">", "<"]
        .into_iter()
        .find_map(|operator| value.strip_prefix(operator))
        .unwrap_or(value);
    if rest.contains('+') {
        return false;
    }
    let (core, prerelease) = match rest.split_once('-') {
        Some((head, prerelease)) => (head, Some(prerelease)),
        None => (rest, None),
    };
    if prerelease.is_some_and(|prerelease| !is_dot_separated_alphanumeric(prerelease)) {
        return false;
    }
    is_version_core(core)
}

/// `[a-z0-9][a-z0-9._-]*` — one npm name segment (scope or bare name).
fn is_package_name_segment(value: &str) -> bool {
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else { return false };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_' || b == b'-')
}

/// TS `packageNamePattern`: `(@scope/)?name`, both segments lowercase npm names.
#[must_use]
pub fn is_package_name(value: &str) -> bool {
    match value.strip_prefix('@') {
        Some(scoped) => match scoped.split_once('/') {
            Some((scope, name)) => is_package_name_segment(scope) && is_package_name_segment(name),
            None => false,
        },
        None => is_package_name_segment(value),
    }
}

// ---------------------------------------------------------------------------
// Field assertions
// ---------------------------------------------------------------------------

fn invalid(code: CredentialModelErrorCode, path: impl Into<String>, detail: &str) -> CredentialModelError {
    CredentialModelError::new(code, path, detail)
}

/// TS `assertIdentifier`: a non-empty string with no surrounding whitespace.
fn assert_identifier(value: &str, path: impl Into<String>) -> Result<(), CredentialModelError> {
    if value.is_empty() || value.trim() != value {
        return Err(invalid(
            CredentialModelErrorCode::InvalidIdentifier,
            path,
            "must be a non-empty trimmed string",
        ));
    }
    Ok(())
}

/// TS `assertVersion`.
fn assert_version(value: &str, path: impl Into<String>) -> Result<(), CredentialModelError> {
    if !is_semantic_version(value) {
        return Err(invalid(
            CredentialModelErrorCode::InvalidVersion,
            path,
            "must be a semantic version",
        ));
    }
    Ok(())
}

/// TS `assertUniqueIds`, specialised to the typed descriptor lists.
fn assert_unique_ids<'a, I>(ids: I, path: &str) -> Result<(), CredentialModelError>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen: HashSet<&str> = HashSet::new();
    for (index, id) in ids.into_iter().enumerate() {
        assert_identifier(id, format!("{path}[{index}].id"))?;
        if !seen.insert(id) {
            return Err(invalid(
                CredentialModelErrorCode::DuplicateId,
                format!("{path}[{index}].id"),
                &format!("duplicates '{id}'"),
            ));
        }
    }
    Ok(())
}

fn assert_codec(descriptor: &crate::types::CodecDescriptor, name: &str) -> Result<(), CredentialModelError> {
    let media_type = descriptor.media_type.as_str();
    if media_type.is_empty() || media_type.trim() != media_type {
        return Err(invalid(
            CredentialModelErrorCode::InvalidCodec,
            name,
            "must declare a media type and encode/decode functions",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Public validators
// ---------------------------------------------------------------------------

/// TS `assertCredentialCompositionManifest`.
///
/// # Errors
///
/// Returns the first [`CredentialModelError`] encountered, in the same order the
/// TypeScript validator raises them.
pub fn assert_credential_composition_manifest(
    manifest: &CredentialCompositionManifest,
) -> Result<(), CredentialModelError> {
    if manifest.format_version != CredentialCompositionManifest::FORMAT_VERSION {
        return Err(invalid(
            CredentialModelErrorCode::InvalidDescriptor,
            "composition",
            "must use formatVersion 1 and declare a packages array",
        ));
    }

    let mut names: HashSet<&str> = HashSet::new();
    for (index, requirement) in manifest.packages.iter().enumerate() {
        assert_package_requirement(requirement, index, &mut names)?;
    }
    Ok(())
}

fn assert_package_requirement<'a>(
    requirement: &'a CredentialPackageRequirement,
    index: usize,
    names: &mut HashSet<&'a str>,
) -> Result<(), CredentialModelError> {
    let path = format!("composition.packages[{index}]");

    if !is_package_name(&requirement.name) {
        return Err(invalid(
            CredentialModelErrorCode::InvalidPackageRequirement,
            format!("{path}.name"),
            "must be a valid npm package name",
        ));
    }
    if !is_package_version(&requirement.version) {
        return Err(invalid(
            CredentialModelErrorCode::InvalidPackageRequirement,
            format!("{path}.version"),
            "must be a registry-resolvable exact or bounded semantic version",
        ));
    }
    if !names.insert(requirement.name.as_str()) {
        return Err(invalid(
            CredentialModelErrorCode::DuplicateId,
            format!("{path}.name"),
            &format!("duplicates '{}'", requirement.name),
        ));
    }

    for (export_index, export_path) in requirement.exports.iter().flatten().enumerate() {
        let export_field = format!("{path}.exports[{export_index}]");
        assert_identifier(export_path, export_field.clone())?;
        if export_path != "." && !export_path.starts_with("./") {
            return Err(invalid(
                CredentialModelErrorCode::InvalidPackageRequirement,
                export_field,
                "must be '.' or an explicit package subpath starting with './'",
            ));
        }
    }
    Ok(())
}

fn assert_schema(schema: &CredentialSchemaDescriptor) -> Result<(), CredentialModelError> {
    assert_identifier(&schema.id, "schema.id")?;
    assert_version(&schema.version, "schema.version")?;

    if schema.credential_types.is_empty() {
        return Err(invalid(
            CredentialModelErrorCode::InvalidDescriptor,
            "schema.credentialTypes",
            "must contain at least one credential type",
        ));
    }
    for (index, credential_type) in schema.credential_types.iter().enumerate() {
        assert_identifier(credential_type, format!("schema.credentialTypes[{index}]"))?;
    }

    assert_unique_ids(schema.claims.iter().map(|claim| claim.id.as_str()), "schema.claims")?;
    for (index, claim) in schema.claims.iter().enumerate() {
        if let Some(value_type) = &claim.value_type {
            assert_identifier(value_type, format!("schema.claims[{index}].valueType"))?;
        }
        if claim.path.is_empty() {
            return Err(invalid(
                CredentialModelErrorCode::InvalidDescriptor,
                format!("schema.claims[{index}].path"),
                "must contain at least one path segment",
            ));
        }
        for (path_index, segment) in claim.path.iter().enumerate() {
            assert_identifier(segment, format!("schema.claims[{index}].path[{path_index}]"))?;
        }
    }
    Ok(())
}

/// TS `assertCredentialFamilyDefinition`.
///
/// # Errors
///
/// Returns the first [`CredentialModelError`] encountered, in the same order the
/// TypeScript validator raises them.
pub fn assert_credential_family_definition(
    definition: &CredentialFamilyDefinition,
) -> Result<(), CredentialModelError> {
    assert_identifier(&definition.id, "id")?;
    assert_version(&definition.version, "version")?;
    assert_schema(&definition.schema)?;

    assert_unique_ids(definition.capabilities.iter().map(|c| c.id.as_str()), "capabilities")?;
    assert_unique_ids(definition.artifacts.iter().map(|a| a.id.as_str()), "artifacts")?;

    for (index, capability) in definition.capabilities.iter().enumerate() {
        if let Some(version) = &capability.version {
            assert_version(version, format!("capabilities[{index}].version"))?;
        }
    }
    for (index, artifact) in definition.artifacts.iter().enumerate() {
        assert_identifier(&artifact.media_type, format!("artifacts[{index}].mediaType"))?;
    }

    assert_credential_composition_manifest(&definition.composition)?;
    assert_codec(&definition.credential_codec, "credentialCodec")?;
    assert_codec(&definition.presentation_codec, "presentationCodec")?;
    Ok(())
}

/// TS `defineCredentialFamily`: validate and hand the definition back.
///
/// # Errors
///
/// Propagates any [`CredentialModelError`] from
/// [`assert_credential_family_definition`].
pub fn define_credential_family(
    definition: CredentialFamilyDefinition,
) -> Result<CredentialFamilyDefinition, CredentialModelError> {
    assert_credential_family_definition(&definition)?;
    Ok(definition)
}

impl CredentialCompositionManifest {
    /// Validate this manifest — see [`assert_credential_composition_manifest`].
    ///
    /// # Errors
    ///
    /// Returns the first [`CredentialModelError`] encountered.
    pub fn validate(&self) -> Result<(), CredentialModelError> {
        assert_credential_composition_manifest(self)
    }
}

impl CredentialFamilyDefinition {
    /// Validate this definition — see [`assert_credential_family_definition`].
    ///
    /// # Errors
    ///
    /// Returns the first [`CredentialModelError`] encountered.
    pub fn validate(&self) -> Result<(), CredentialModelError> {
        assert_credential_family_definition(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        ClaimDisclosure, CodecDescriptor, CredentialCapabilityDescriptor, CredentialCapabilityKind,
        CredentialClaimDescriptor, ProofArtifactPurpose, ProofArtifactRequirement,
    };

    // -- pattern scanners ---------------------------------------------------

    #[test]
    fn semantic_version_accepts_the_regex_accept_class() {
        for accepted in [
            "0.0.0",
            "1.2.3",
            "10.20.30",
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-0.3.7",
            "1.0.0-x-y-z",
            "1.0.0+build",
            "1.0.0+build.1",
            "1.0.0-alpha+build",
        ] {
            assert!(is_semantic_version(accepted), "{accepted} should be accepted");
        }
    }

    #[test]
    fn semantic_version_rejects_the_regex_reject_class() {
        for rejected in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "v1.2.3",
            "1.2.x",
            "1.2.3-",
            "1.2.3+",
            "1.2.3-alpha_1",
            "1.2.3+build_1",
            "^1.2.3",
            " 1.2.3",
        ] {
            assert!(!is_semantic_version(rejected), "{rejected:?} should be rejected");
        }
    }

    #[test]
    fn package_version_accepts_bare_and_bounded_ranges() {
        for accepted in [
            "1.2.3",
            "~1.2.3",
            "^1.2.3",
            ">=1.2.3",
            "<=1.2.3",
            ">1.2.3",
            "<1.2.3",
            "1.2.3-rc.1",
        ] {
            assert!(is_package_version(accepted), "{accepted} should be accepted");
        }
    }

    #[test]
    fn package_version_rejects_build_metadata_and_non_registry_specifiers() {
        for rejected in [
            "1.2.3+build",
            "file:../local",
            "link:../local",
            "workspace:*",
            "git:git@example.com/x.git",
            "https://example.com/x.tgz",
            "*",
            "",
            ">=~1.2.3",
            "1.2.3-",
        ] {
            assert!(!is_package_version(rejected), "{rejected:?} should be rejected");
        }
    }

    #[test]
    fn package_name_accepts_scoped_and_unscoped_names() {
        for accepted in [
            "pkg",
            "p",
            "0pkg",
            "my.pkg",
            "my_pkg",
            "my-pkg",
            "@scope/pkg",
            "@0scope/my-pkg.v2",
        ] {
            assert!(is_package_name(accepted), "{accepted} should be accepted");
        }
    }

    #[test]
    fn package_name_rejects_uppercase_bad_scopes_and_stray_slashes() {
        for rejected in [
            "",
            "Pkg",
            "pKg",
            "-pkg",
            ".pkg",
            "_pkg",
            "@scope",
            "@/pkg",
            "@scope/",
            "@Scope/pkg",
            "scope/pkg",
            "@scope/pkg/extra",
            "pkg name",
        ] {
            assert!(!is_package_name(rejected), "{rejected:?} should be rejected");
        }
    }

    // -- fixtures -----------------------------------------------------------

    fn claim(id: &str) -> CredentialClaimDescriptor {
        CredentialClaimDescriptor {
            id: id.into(),
            path: vec!["credentialSubject".into(), id.into()],
            disclosure: ClaimDisclosure::Public,
            required: true,
            value_type: None,
        }
    }

    fn manifest() -> CredentialCompositionManifest {
        CredentialCompositionManifest::new(vec![CredentialPackageRequirement {
            name: "@midnight-ntwrk/midnight-did-credentials".into(),
            version: "^0.1.0".into(),
            exports: Some(vec![".".into(), "./contract".into()]),
        }])
    }

    fn definition() -> CredentialFamilyDefinition {
        CredentialFamilyDefinition {
            id: "birth".into(),
            version: "0.1.0".into(),
            schema: CredentialSchemaDescriptor {
                id: "birth-schema".into(),
                version: "1.0.0".into(),
                credential_types: vec!["BirthCredential".into()],
                claims: vec![claim("dateOfBirth"), claim("placeOfBirth")],
            },
            capabilities: vec![CredentialCapabilityDescriptor {
                id: "holder-secret".into(),
                kind: CredentialCapabilityKind::HolderBinding,
                version: Some("0.1.0".into()),
                required: true,
            }],
            artifacts: vec![ProofArtifactRequirement {
                id: "issue.prover".into(),
                media_type: "application/octet-stream".into(),
                purpose: ProofArtifactPurpose::Prover,
                optional: None,
            }],
            composition: manifest(),
            credential_codec: CodecDescriptor::new("application/vc+json"),
            presentation_codec: CodecDescriptor::new("application/vp+json"),
        }
    }

    #[track_caller]
    fn expect_error(
        result: Result<(), CredentialModelError>,
        code: CredentialModelErrorCode,
        path: &str,
    ) -> CredentialModelError {
        let error = result.expect_err("expected a validation failure");
        assert_eq!(error.code, code, "error code");
        assert_eq!(error.path, path, "error path");
        error
    }

    // -- happy paths --------------------------------------------------------

    #[test]
    fn valid_definition_passes_and_define_returns_it() {
        let expected = definition();
        assert!(assert_credential_family_definition(&expected).is_ok());
        assert!(expected.validate().is_ok());
        assert_eq!(define_credential_family(definition()).expect("valid family"), expected);
    }

    #[test]
    fn manifest_without_exports_is_valid() {
        let manifest = CredentialCompositionManifest::new(vec![CredentialPackageRequirement {
            name: "pkg".into(),
            version: "1.0.0".into(),
            exports: None,
        }]);
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn empty_claim_and_capability_lists_are_valid() {
        let mut definition = definition();
        definition.schema.claims.clear();
        definition.capabilities.clear();
        definition.artifacts.clear();
        assert!(definition.validate().is_ok());
    }

    // -- definition-level failures -----------------------------------------

    #[test]
    fn blank_family_id_is_an_invalid_identifier() {
        let mut definition = definition();
        definition.id = "  ".into();
        expect_error(definition.validate(), CredentialModelErrorCode::InvalidIdentifier, "id");
    }

    #[test]
    fn untrimmed_family_id_is_an_invalid_identifier() {
        let mut definition = definition();
        definition.id = " birth".into();
        let error = expect_error(definition.validate(), CredentialModelErrorCode::InvalidIdentifier, "id");
        assert_eq!(error.detail, "must be a non-empty trimmed string");
    }

    #[test]
    fn non_semver_family_version_is_an_invalid_version() {
        let mut definition = definition();
        definition.version = "0.1".into();
        let error = expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidVersion,
            "version",
        );
        assert_eq!(error.detail, "must be a semantic version");
    }

    #[test]
    fn blank_schema_id_is_reported_under_schema_id() {
        let mut definition = definition();
        definition.schema.id = String::new();
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "schema.id",
        );
    }

    #[test]
    fn non_semver_schema_version_is_reported_under_schema_version() {
        let mut definition = definition();
        definition.schema.version = "latest".into();
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidVersion,
            "schema.version",
        );
    }

    #[test]
    fn empty_credential_types_is_an_invalid_descriptor() {
        let mut definition = definition();
        definition.schema.credential_types.clear();
        let error = expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidDescriptor,
            "schema.credentialTypes",
        );
        assert_eq!(error.detail, "must contain at least one credential type");
    }

    #[test]
    fn blank_credential_type_is_reported_with_its_index() {
        let mut definition = definition();
        definition.schema.credential_types.push(String::new());
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "schema.credentialTypes[1]",
        );
    }

    #[test]
    fn duplicate_claim_ids_are_reported_at_the_second_occurrence() {
        let mut definition = definition();
        definition.schema.claims = vec![claim("dateOfBirth"), claim("dateOfBirth")];
        let error = expect_error(
            definition.validate(),
            CredentialModelErrorCode::DuplicateId,
            "schema.claims[1].id",
        );
        assert_eq!(error.detail, "duplicates 'dateOfBirth'");
    }

    #[test]
    fn blank_claim_id_is_an_invalid_identifier() {
        let mut definition = definition();
        definition.schema.claims = vec![claim("")];
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "schema.claims[0].id",
        );
    }

    #[test]
    fn empty_claim_path_is_an_invalid_descriptor() {
        let mut definition = definition();
        definition.schema.claims[1].path.clear();
        let error = expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidDescriptor,
            "schema.claims[1].path",
        );
        assert_eq!(error.detail, "must contain at least one path segment");
    }

    #[test]
    fn blank_claim_path_segment_is_reported_with_both_indices() {
        let mut definition = definition();
        definition.schema.claims[0].path[1] = " ".into();
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "schema.claims[0].path[1]",
        );
    }

    #[test]
    fn blank_claim_value_type_is_an_invalid_identifier() {
        let mut definition = definition();
        definition.schema.claims[0].value_type = Some(String::new());
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "schema.claims[0].valueType",
        );
    }

    #[test]
    fn duplicate_capability_ids_are_rejected() {
        let mut definition = definition();
        let capability = definition.capabilities[0].clone();
        definition.capabilities.push(capability);
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::DuplicateId,
            "capabilities[1].id",
        );
    }

    #[test]
    fn non_semver_capability_version_is_rejected() {
        let mut definition = definition();
        definition.capabilities[0].version = Some("0.1".into());
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidVersion,
            "capabilities[0].version",
        );
    }

    #[test]
    fn capability_without_a_version_is_accepted() {
        let mut definition = definition();
        definition.capabilities[0].version = None;
        assert!(definition.validate().is_ok());
    }

    #[test]
    fn duplicate_artifact_ids_are_rejected() {
        let mut definition = definition();
        let artifact = definition.artifacts[0].clone();
        definition.artifacts.push(artifact);
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::DuplicateId,
            "artifacts[1].id",
        );
    }

    #[test]
    fn blank_artifact_media_type_is_rejected() {
        let mut definition = definition();
        definition.artifacts[0].media_type = " ".into();
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "artifacts[0].mediaType",
        );
    }

    // -- codec failures -----------------------------------------------------

    #[test]
    fn blank_credential_codec_media_type_is_an_invalid_codec() {
        let mut definition = definition();
        definition.credential_codec = CodecDescriptor::new("");
        let error = expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidCodec,
            "credentialCodec",
        );
        assert_eq!(error.detail, "must declare a media type and encode/decode functions");
    }

    #[test]
    fn untrimmed_presentation_codec_media_type_is_an_invalid_codec() {
        let mut definition = definition();
        definition.presentation_codec = CodecDescriptor::new("application/vp+json ");
        expect_error(
            definition.validate(),
            CredentialModelErrorCode::InvalidCodec,
            "presentationCodec",
        );
    }

    // -- manifest failures --------------------------------------------------

    #[test]
    fn wrong_format_version_is_an_invalid_descriptor() {
        let mut manifest = manifest();
        manifest.format_version = 2;
        let error = expect_error(
            manifest.validate(),
            CredentialModelErrorCode::InvalidDescriptor,
            "composition",
        );
        assert_eq!(error.detail, "must use formatVersion 1 and declare a packages array");
    }

    #[test]
    fn invalid_package_name_is_reported_under_the_manifest_path() {
        let mut manifest = manifest();
        manifest.packages[0].name = "Bad Name".into();
        let error = expect_error(
            manifest.validate(),
            CredentialModelErrorCode::InvalidPackageRequirement,
            "composition.packages[0].name",
        );
        assert_eq!(error.detail, "must be a valid npm package name");
    }

    #[test]
    fn non_registry_package_version_is_rejected() {
        let mut manifest = manifest();
        manifest.packages[0].version = "workspace:*".into();
        let error = expect_error(
            manifest.validate(),
            CredentialModelErrorCode::InvalidPackageRequirement,
            "composition.packages[0].version",
        );
        assert_eq!(
            error.detail,
            "must be a registry-resolvable exact or bounded semantic version"
        );
    }

    #[test]
    fn duplicate_package_names_are_rejected() {
        let mut manifest = manifest();
        let requirement = manifest.packages[0].clone();
        manifest.packages.push(requirement);
        let error = expect_error(
            manifest.validate(),
            CredentialModelErrorCode::DuplicateId,
            "composition.packages[1].name",
        );
        assert_eq!(error.detail, "duplicates '@midnight-ntwrk/midnight-did-credentials'");
    }

    #[test]
    fn blank_export_path_is_an_invalid_identifier() {
        let mut manifest = manifest();
        manifest.packages[0].exports = Some(vec![String::new()]);
        expect_error(
            manifest.validate(),
            CredentialModelErrorCode::InvalidIdentifier,
            "composition.packages[0].exports[0]",
        );
    }

    #[test]
    fn relative_export_path_must_start_with_dot_slash() {
        let mut manifest = manifest();
        manifest.packages[0].exports = Some(vec![".".into(), "contract".into()]);
        let error = expect_error(
            manifest.validate(),
            CredentialModelErrorCode::InvalidPackageRequirement,
            "composition.packages[0].exports[1]",
        );
        assert_eq!(
            error.detail,
            "must be '.' or an explicit package subpath starting with './'"
        );
    }

    #[test]
    fn manifest_failures_surface_through_the_family_validator() {
        let mut definition = definition();
        definition.composition.format_version = 0;
        expect_error(
            define_credential_family(definition).map(|_| ()),
            CredentialModelErrorCode::InvalidDescriptor,
            "composition",
        );
    }
}
