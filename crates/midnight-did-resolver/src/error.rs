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

//! Typed resolution errors + the W3C resolution-envelope error codes,
//! matching the TS service's wire vocabulary
//! (`did-resolver-service/src/resolution-errors.ts`) — but classified
//! by type, not by error-message string matching.

use serde::Serialize;

/// Wire error codes of the `didResolutionMetadata.error` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolutionErrorCode {
    /// The DID does not resolve to a known contract (HTTP 404).
    NotFound,
    /// The input is not a valid `did:midnight` (HTTP 400).
    InvalidDid,
    /// The DID's network doesn't match the service's expected network,
    /// or the DID targets an unresolvable (offchain) network (HTTP 400).
    NetworkMismatch,
    /// Anything else — indexer/transport/decode failures (HTTP 500).
    InternalError,
}

impl ResolutionErrorCode {
    /// The HTTP status the TS service maps this code to.
    pub fn http_status(self) -> u16 {
        match self {
            Self::NotFound => 404,
            Self::InvalidDid | Self::NetworkMismatch => 400,
            Self::InternalError => 500,
        }
    }
}

/// Internal resolution failure with the detail kept out of the wire
/// envelope (the TS service also returns only the code).
#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {detail}")]
pub struct ResolutionError {
    /// Wire code.
    pub code: ResolutionErrorCode,
    /// Human detail for logs.
    pub detail: String,
}

impl ResolutionError {
    /// Build an error with the given code and log detail.
    pub fn new(code: ResolutionErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}
