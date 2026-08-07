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

//! Indexer-backed **read-only** [`Backend`] for the Midnight DID method.
//!
//! This is "Approach A" of the LiveBackend track (issue #4): resolution
//! goes live by reading contract state from the Midnight indexer's
//! GraphQL API — the same `contractAction(address){state}` query the TS
//! `indexer-public-data-provider` issues — and decoding it through
//! [`midnight_did_runtime::state_decode`]. Writes stay unsupported
//! (`submit_tx` → [`BackendError::ReadOnly`]); the wallet + proof-server
//! write path is tracked separately (issue #9).
//!
//! ```no_run
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! use midnight_did_indexer::{IndexerBackend, IndexerClient};
//!
//! let client = IndexerClient::new("http://127.0.0.1:8088/api/v3/graphql")?;
//! let backend = IndexerBackend::new(client, "00aabb…contract-address-hex…");
//! // hand `backend` to `Contract::new(backend, addr, network)` and resolve.
//! # Ok(()) }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

use async_trait::async_trait;
use midnight_did_runtime::backend::{Backend, BackendError, BuiltTx, FinalizedTxData};
use midnight_did_runtime::contract_call::DidLedgerSnapshot;
use midnight_did_runtime::state_decode::{charged_state_from_bytes, decode_ledger_snapshot};
use serde::Deserialize;
use serde_json::json;

/// The exact query the TS `indexer-public-data-provider` (4.x) issues
/// for the latest contract state.
const CONTRACT_STATE_QUERY: &str = "query CONTRACT_STATE_QUERY($address: HexEncoded!, $offset: ContractActionOffset) {\n  contractAction(address: $address, offset: $offset) {\n    state\n  }\n}";

/// Errors from the indexer HTTP/GraphQL exchange.
#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    /// Transport-level failure (connect, timeout, non-2xx).
    #[error("indexer transport: {0}")]
    Transport(String),
    /// The indexer answered, but with GraphQL errors.
    #[error("indexer GraphQL: {0}")]
    GraphQl(String),
    /// The response body didn't match the expected shape.
    #[error("indexer response: {0}")]
    Malformed(String),
}

impl From<IndexerError> for BackendError {
    fn from(e: IndexerError) -> Self {
        BackendError::Network(e.to_string())
    }
}

/// Minimal GraphQL-over-HTTP client for the Midnight indexer.
#[derive(Debug, Clone)]
pub struct IndexerClient {
    http: reqwest::Client,
    graphql_url: String,
}

impl IndexerClient {
    /// Build a client for the indexer's GraphQL endpoint
    /// (e.g. `http://127.0.0.1:8088/api/v3/graphql`).
    pub fn new(graphql_url: impl Into<String>) -> Result<Self, IndexerError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        Ok(Self {
            http,
            graphql_url: graphql_url.into(),
        })
    }

    /// The configured GraphQL endpoint URL.
    pub fn graphql_url(&self) -> &str {
        &self.graphql_url
    }

    /// Fetch the latest serialized contract state for `address_hex`.
    ///
    /// Returns `Ok(None)` when the indexer knows no contract action for
    /// the address (the TS provider's `null` — maps to *notFound*).
    pub async fn contract_state_bytes(&self, address_hex: &str) -> Result<Option<Vec<u8>>, IndexerError> {
        let body = json!({
            "query": CONTRACT_STATE_QUERY,
            "variables": { "address": address_hex, "offset": null },
        });
        let response = self
            .http
            .post(&self.graphql_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexerError::Transport(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(IndexerError::Transport(format!("HTTP {status}")));
        }
        let payload: GraphQlResponse = response
            .json()
            .await
            .map_err(|e| IndexerError::Malformed(e.to_string()))?;
        parse_contract_state_response(payload)
    }
}

/// Read-only [`Backend`] over an [`IndexerClient`] + contract address.
#[derive(Debug, Clone)]
pub struct IndexerBackend {
    client: IndexerClient,
    address_hex: String,
}

impl IndexerBackend {
    /// Bind a client to one contract address (hex, no `0x` prefix).
    pub fn new(client: IndexerClient, address_hex: impl Into<String>) -> Self {
        Self {
            client,
            address_hex: address_hex.into(),
        }
    }

    /// The bound contract address (hex).
    pub fn address_hex(&self) -> &str {
        &self.address_hex
    }

    async fn fetch_state_bytes(&self) -> Result<Vec<u8>, BackendError> {
        self.client
            .contract_state_bytes(&self.address_hex)
            .await
            .map_err(BackendError::from)?
            .ok_or_else(|| {
                BackendError::Other(format!(
                    "no contract state for address {} (not deployed?)",
                    self.address_hex
                ))
            })
    }
}

#[async_trait]
impl Backend for IndexerBackend {
    async fn submit_tx(&self, _tx: BuiltTx) -> Result<FinalizedTxData, BackendError> {
        Err(BackendError::ReadOnly)
    }

    async fn read_state(
        &self,
    ) -> Result<midnight_did_runtime::backend::RawChargedState<midnight_did_runtime::backend::RawDb>, BackendError>
    {
        charged_state_from_bytes(&self.fetch_state_bytes().await?)
    }

    async fn read_snapshot(&self) -> Result<DidLedgerSnapshot, BackendError> {
        let state = charged_state_from_bytes(&self.fetch_state_bytes().await?)?;
        decode_ledger_snapshot(&state)
    }
}

// ─────────────────────────────────────────────────────────────────────
// Response parsing (pure — unit-tested without HTTP)
// ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GraphQlResponse {
    #[serde(default)]
    data: Option<ContractActionData>,
    #[serde(default)]
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Debug, Deserialize)]
struct ContractActionData {
    #[serde(rename = "contractAction")]
    contract_action: Option<ContractAction>,
}

#[derive(Debug, Deserialize)]
struct ContractAction {
    state: String,
}

#[derive(Debug, Deserialize)]
struct GraphQlError {
    message: String,
}

fn parse_contract_state_response(payload: GraphQlResponse) -> Result<Option<Vec<u8>>, IndexerError> {
    if let Some(errors) = payload.errors
        && !errors.is_empty()
    {
        let joined = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; ");
        return Err(IndexerError::GraphQl(joined));
    }
    let Some(data) = payload.data else {
        return Err(IndexerError::Malformed("response has neither data nor errors".into()));
    };
    let Some(action) = data.contract_action else {
        return Ok(None); // unknown address → notFound
    };
    let bytes = hex::decode(action.state.trim_start_matches("0x"))
        .map_err(|e| IndexerError::Malformed(format!("state field is not hex: {e}")))?;
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json_str: &str) -> Result<Option<Vec<u8>>, IndexerError> {
        let payload: GraphQlResponse = serde_json::from_str(json_str).expect("test JSON parses");
        parse_contract_state_response(payload)
    }

    #[test]
    fn parses_state_hex() {
        let got = parse(r#"{"data":{"contractAction":{"state":"00ff10"}}}"#).unwrap();
        assert_eq!(got, Some(vec![0x00, 0xff, 0x10]));
    }

    #[test]
    fn null_contract_action_is_none() {
        let got = parse(r#"{"data":{"contractAction":null}}"#).unwrap();
        assert_eq!(got, None);
    }

    #[test]
    fn graphql_errors_are_surfaced() {
        let err = parse(r#"{"errors":[{"message":"boom"},{"message":"bang"}]}"#).unwrap_err();
        assert!(matches!(err, IndexerError::GraphQl(m) if m == "boom; bang"));
    }

    #[test]
    fn non_hex_state_is_malformed() {
        let err = parse(r#"{"data":{"contractAction":{"state":"zz"}}}"#).unwrap_err();
        assert!(matches!(err, IndexerError::Malformed(_)));
    }

    #[test]
    fn empty_body_is_malformed() {
        let err = parse(r#"{}"#).unwrap_err();
        assert!(matches!(err, IndexerError::Malformed(_)));
    }

    #[tokio::test]
    async fn submit_tx_is_read_only() {
        let client = IndexerClient::new("http://127.0.0.1:1/api/v3/graphql").unwrap();
        let backend = IndexerBackend::new(client, "00");
        let err = backend.submit_tx(BuiltTx { bytes: vec![] }).await.unwrap_err();
        assert!(matches!(err, BackendError::ReadOnly));
    }
}
