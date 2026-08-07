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

//! The resolution pipeline: DID string → network guards → indexer
//! fetch → state decode → W3C document + metadata.
//!
//! Deliberately bypasses the `Contract<Backend>` wrapper and composes
//! the typed layers directly (`IndexerClient` →
//! `state_decode` → `ledger_state_to_did_document`) so every failure
//! mode is classified by **type** — the TS service's weakest point is
//! its error-message string matching.

use midnight_did_api::resolution::{ResolvedMidnightDid, ledger_state_to_did_document, ledger_state_to_metadata};
use midnight_did_indexer::IndexerClient;
use midnight_did_method::midnight_did::{
    MidnightNetwork, MidnightSubjectId, parse_midnight_did, parse_midnight_did_string,
};
use midnight_did_runtime::state_decode::{charged_state_from_bytes, decode_ledger_snapshot};

use crate::config::ResolverConfig;
use crate::endpoint_policy::validate_override;
use crate::error::{ResolutionError, ResolutionErrorCode};

/// Stateless resolver over a configured default indexer.
#[derive(Debug, Clone)]
pub struct ResolverService {
    config: ResolverConfig,
}

impl ResolverService {
    /// Build a service from config.
    pub fn new(config: ResolverConfig) -> Self {
        Self { config }
    }

    /// The service configuration.
    pub fn config(&self) -> &ResolverConfig {
        &self.config
    }

    /// Resolve `did` — optionally against a caller-supplied indexer URL
    /// (validated by the endpoint policy).
    pub async fn resolve(
        &self,
        did: &str,
        indexer_override: Option<&str>,
    ) -> Result<ResolvedMidnightDid, ResolutionError> {
        let fut = self.resolve_inner(did, indexer_override);
        match tokio::time::timeout(std::time::Duration::from_millis(self.config.timeout_ms), fut).await {
            Ok(result) => result,
            Err(_) => Err(ResolutionError::new(
                ResolutionErrorCode::InternalError,
                format!("resolution timed out after {}ms", self.config.timeout_ms),
            )),
        }
    }

    async fn resolve_inner(
        &self,
        did: &str,
        indexer_override: Option<&str>,
    ) -> Result<ResolvedMidnightDid, ResolutionError> {
        use ResolutionErrorCode::*;

        // 1. Parse.
        let did_string =
            parse_midnight_did_string(did).map_err(|e| ResolutionError::new(InvalidDid, format!("{did}: {e:?}")))?;
        let (network, subject) =
            parse_midnight_did(&did_string).map_err(|e| ResolutionError::new(InvalidDid, format!("{did}: {e:?}")))?;

        // 2. Network guards: offchain DIDs are not chain-resolvable
        //    here; an expected-network mismatch is rejected (both mirror
        //    the TS service).
        let address_hex = subject.to_hex();
        if !matches!(subject, MidnightSubjectId::Contract(_)) {
            return Err(ResolutionError::new(
                NetworkMismatch,
                "offchain DIDs are not resolvable against an indexer",
            ));
        }
        if matches!(network, MidnightNetwork::Offchain) {
            return Err(ResolutionError::new(NetworkMismatch, "offchain network"));
        }
        if let Some(expected) = self.config.expected_network
            && network != expected
        {
            return Err(ResolutionError::new(
                NetworkMismatch,
                format!("DID network {network:?} != expected {expected:?}"),
            ));
        }

        // 3. Endpoint selection (override goes through the SSRF policy).
        let indexer_url = match indexer_override {
            Some(raw) => validate_override(raw, self.config.allow_private_indexer_overrides)
                .map_err(|reason| ResolutionError::new(InvalidDid, reason))?,
            None => self.config.indexer_url.clone(),
        };

        // 4. Fetch + decode + project — every step typed.
        let client = IndexerClient::new(indexer_url).map_err(|e| ResolutionError::new(InternalError, e.to_string()))?;
        let bytes = client
            .contract_state_bytes(&address_hex)
            .await
            .map_err(|e| ResolutionError::new(InternalError, e.to_string()))?
            .ok_or_else(|| ResolutionError::new(NotFound, format!("no contract state for {address_hex}")))?;
        let state = charged_state_from_bytes(&bytes).map_err(|e| ResolutionError::new(InternalError, e.to_string()))?;
        let snapshot =
            decode_ledger_snapshot(&state).map_err(|e| ResolutionError::new(InternalError, e.to_string()))?;

        let did_document = ledger_state_to_did_document(&snapshot, network, &address_hex)
            .map_err(|e| ResolutionError::new(InternalError, e.to_string()))?;
        let did_document_metadata = ledger_state_to_metadata(&snapshot);
        Ok(ResolvedMidnightDid {
            did_document,
            did_document_metadata,
        })
    }
}
