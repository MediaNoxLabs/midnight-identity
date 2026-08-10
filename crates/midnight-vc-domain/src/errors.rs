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

//! Validation error type — port of `packages/core/model/src/errors.ts`.
//!
//! The TypeScript class derives its message as `` `${path}: ${message}` ``; the
//! [`std::fmt::Display`] impl here reproduces that spelling exactly so error
//! strings compare equal across the two implementations.

use serde::{Deserialize, Serialize};

/// Machine-readable classification of a credential-model validation failure.
///
/// Wire spellings match the TypeScript `CredentialModelErrorCode` union
/// (`SCREAMING_SNAKE_CASE` string literals).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CredentialModelErrorCode {
    /// A value that must be a non-empty, trimmed string was not.
    InvalidIdentifier,
    /// A value that must be a semantic version was not.
    InvalidVersion,
    /// A descriptor field was structurally invalid or out of its allowed set.
    InvalidDescriptor,
    /// Two sibling entries share an `id` (or a package name).
    DuplicateId,
    /// A composition-manifest package requirement was invalid.
    InvalidPackageRequirement,
    /// A codec declaration was invalid (e.g. a blank media type).
    InvalidCodec,
}

impl CredentialModelErrorCode {
    /// The TypeScript string literal for this code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidIdentifier => "INVALID_IDENTIFIER",
            Self::InvalidVersion => "INVALID_VERSION",
            Self::InvalidDescriptor => "INVALID_DESCRIPTOR",
            Self::DuplicateId => "DUPLICATE_ID",
            Self::InvalidPackageRequirement => "INVALID_PACKAGE_REQUIREMENT",
            Self::InvalidCodec => "INVALID_CODEC",
        }
    }
}

/// A credential-model validation failure.
///
/// Mirrors the TypeScript `CredentialModelError`: a `code`, the dotted/indexed
/// `path` of the offending value, and a human-readable `detail`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {detail}")]
pub struct CredentialModelError {
    /// Machine-readable classification.
    pub code: CredentialModelErrorCode,
    /// Path of the offending value, e.g. `schema.claims[2].disclosure`.
    pub path: String,
    /// Human-readable explanation, e.g. `must be a semantic version`.
    pub detail: String,
}

impl CredentialModelError {
    /// Construct an error, mirroring the TypeScript constructor arity.
    #[must_use]
    pub fn new(code: CredentialModelErrorCode, path: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CODES: [(CredentialModelErrorCode, &str); 6] = [
        (CredentialModelErrorCode::InvalidIdentifier, "INVALID_IDENTIFIER"),
        (CredentialModelErrorCode::InvalidVersion, "INVALID_VERSION"),
        (CredentialModelErrorCode::InvalidDescriptor, "INVALID_DESCRIPTOR"),
        (CredentialModelErrorCode::DuplicateId, "DUPLICATE_ID"),
        (
            CredentialModelErrorCode::InvalidPackageRequirement,
            "INVALID_PACKAGE_REQUIREMENT",
        ),
        (CredentialModelErrorCode::InvalidCodec, "INVALID_CODEC"),
    ];

    #[test]
    fn error_code_as_str_matches_typescript_literals() {
        for (code, literal) in ALL_CODES {
            assert_eq!(code.as_str(), literal);
        }
    }

    #[test]
    fn error_code_serializes_to_typescript_literals() {
        for (code, literal) in ALL_CODES {
            let json = serde_json::to_string(&code).expect("serialize code");
            assert_eq!(json, format!("\"{literal}\""));
            let round_tripped: CredentialModelErrorCode = serde_json::from_str(&json).expect("deserialize code");
            assert_eq!(round_tripped, code);
        }
    }

    #[test]
    fn display_reproduces_typescript_message_shape() {
        let error = CredentialModelError::new(
            CredentialModelErrorCode::InvalidVersion,
            "schema.version",
            "must be a semantic version",
        );
        assert_eq!(error.to_string(), "schema.version: must be a semantic version");
        assert_eq!(error.code, CredentialModelErrorCode::InvalidVersion);
        assert_eq!(error.path, "schema.version");
        assert_eq!(error.detail, "must be a semantic version");
    }

    #[test]
    fn errors_compare_and_clone_structurally() {
        let error = CredentialModelError::new(CredentialModelErrorCode::DuplicateId, "capabilities[1].id", "dup");
        assert_eq!(error.clone(), error);
        assert!(format!("{error:?}").contains("DuplicateId"));
    }
}
