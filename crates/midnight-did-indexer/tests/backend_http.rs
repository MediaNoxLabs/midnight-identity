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

//! `IndexerBackend` read-path tests against a local mock GraphQL server
//! serving REAL captured chain state
//! (`tests/fixtures/state-050-enriched.hex` — same capture as the
//! resolver crate's route tests: a 0.5.0 DID from the standalone stack
//! with a JWK verification method and an authentication relation).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use midnight_did_indexer::{IndexerBackend, IndexerClient};
use midnight_did_runtime::backend::{Backend, BackendError, BuiltTx};
use midnight_did_runtime::state_decode::decode_ledger_snapshot;
use serde_json::{Value, json};

const STATE_HEX: &str = include_str!("fixtures/state-050-enriched.hex");
const ADDR: &str = include_str!("fixtures/state-050-enriched.addr");

fn fixture_addr() -> String {
    ADDR.trim().to_string()
}

/// How the mock indexer answers a `CONTRACT_STATE_QUERY` POST.
#[derive(Clone, Copy)]
enum MockReply {
    /// Fixture state for the fixture address, `contractAction: null`
    /// for any other address.
    Fixture,
    /// A GraphQL `errors` array (HTTP 200).
    GraphQlErrors,
    /// Plain HTTP 500 with a non-GraphQL body.
    Http500,
    /// HTTP 200 whose body is not JSON at all.
    NotJson,
    /// A `state` field that is not hex.
    BadHexState,
    /// Valid hex that is not a serialized `ContractState`.
    GarbageStateBytes,
}

impl MockReply {
    fn respond(self, body: &Value) -> Response {
        match self {
            MockReply::Fixture => {
                let requested = body["variables"]["address"].as_str().unwrap_or_default();
                if requested.eq_ignore_ascii_case(&fixture_addr()) {
                    Json(json!({"data": {"contractAction": {"state": STATE_HEX.trim()}}})).into_response()
                } else {
                    Json(json!({"data": {"contractAction": null}})).into_response()
                }
            }
            MockReply::GraphQlErrors => {
                Json(json!({"errors": [{"message": "boom"}, {"message": "bang"}]})).into_response()
            }
            MockReply::Http500 => (StatusCode::INTERNAL_SERVER_ERROR, "kaboom").into_response(),
            MockReply::NotJson => "this is not json".into_response(),
            MockReply::BadHexState => Json(json!({"data": {"contractAction": {"state": "zz"}}})).into_response(),
            MockReply::GarbageStateBytes => {
                Json(json!({"data": {"contractAction": {"state": "00ff10"}}})).into_response()
            }
        }
    }
}

/// Spawn a one-route GraphQL mock and return its endpoint URL.
async fn spawn_mock(reply: MockReply) -> String {
    let router = Router::new().route(
        "/api/v3/graphql",
        post(move |Json(body): Json<Value>| async move { reply.respond(&body) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://127.0.0.1:{port}/api/v3/graphql")
}

async fn backend_for(reply: MockReply, address: &str) -> IndexerBackend {
    let url = spawn_mock(reply).await;
    IndexerBackend::new(IndexerClient::new(url).unwrap(), address)
}

fn assert_network_err(err: &BackendError, needle: &str) {
    match err {
        BackendError::Network(msg) => {
            assert!(msg.contains(needle), "message {msg:?} does not contain {needle:?}");
        }
        other => panic!("expected Network error containing {needle:?}, got {other:?}"),
    }
}

#[tokio::test]
async fn read_snapshot_decodes_fixture_state() {
    let backend = backend_for(MockReply::Fixture, &fixture_addr()).await;
    let snapshot = backend.read_snapshot().await.expect("fixture snapshot decodes");
    assert!(snapshot.active);
    assert!(!snapshot.deactivated);
    assert_eq!(snapshot.version, 2);
    assert_eq!(snapshot.authentication_relation, vec!["#key-1".to_string()]);
    assert!(snapshot.verification_methods.contains_key("#key-1"));
}

#[tokio::test]
async fn read_state_returns_decodable_charged_state() {
    let backend = backend_for(MockReply::Fixture, &fixture_addr()).await;
    let state = backend.read_state().await.expect("fixture state parses");
    // The raw read path must agree with the snapshot read path.
    let via_state = decode_ledger_snapshot(&state).expect("charged state projects");
    let via_snapshot = backend.read_snapshot().await.unwrap();
    assert_eq!(via_state, via_snapshot);
}

#[tokio::test]
async fn unknown_address_is_backend_other() {
    let backend = backend_for(MockReply::Fixture, &"11".repeat(32)).await;
    let err = backend.read_snapshot().await.unwrap_err();
    match err {
        BackendError::Other(msg) => assert!(msg.contains("no contract state"), "got {msg}"),
        other => panic!("expected Other, got {other:?}"),
    }
}

#[tokio::test]
async fn graphql_errors_surface_as_network_error() {
    let backend = backend_for(MockReply::GraphQlErrors, &fixture_addr()).await;
    let err = backend.read_snapshot().await.unwrap_err();
    assert_network_err(&err, "boom; bang");
}

#[tokio::test]
async fn http_500_is_a_transport_error() {
    let backend = backend_for(MockReply::Http500, &fixture_addr()).await;
    let err = backend.read_state().await.unwrap_err();
    assert_network_err(&err, "HTTP 500");
}

#[tokio::test]
async fn non_json_body_is_malformed() {
    let backend = backend_for(MockReply::NotJson, &fixture_addr()).await;
    let err = backend.read_snapshot().await.unwrap_err();
    assert_network_err(&err, "indexer response");
}

#[tokio::test]
async fn non_hex_state_is_malformed() {
    let backend = backend_for(MockReply::BadHexState, &fixture_addr()).await;
    let err = backend.read_snapshot().await.unwrap_err();
    assert_network_err(&err, "state field is not hex");
}

#[tokio::test]
async fn garbage_state_bytes_are_a_decode_error() {
    // Transport succeeds, hex decodes — but the bytes are not a
    // serialized ContractState, so the failure is typed Decode.
    let backend = backend_for(MockReply::GarbageStateBytes, &fixture_addr()).await;
    let err = backend.read_snapshot().await.unwrap_err();
    assert!(matches!(err, BackendError::Decode(_)), "got {err:?}");
}

#[tokio::test]
async fn connection_refused_is_a_transport_error() {
    // Nothing listens on port 1.
    let client = IndexerClient::new("http://127.0.0.1:1/api/v3/graphql").unwrap();
    let backend = IndexerBackend::new(client, fixture_addr());
    let err = backend.read_snapshot().await.unwrap_err();
    assert_network_err(&err, "indexer transport");
}

#[tokio::test]
async fn submit_tx_stays_read_only_even_with_live_mock() {
    let backend = backend_for(MockReply::Fixture, &fixture_addr()).await;
    let err = backend.submit_tx(BuiltTx { bytes: vec![1] }).await.unwrap_err();
    assert!(matches!(err, BackendError::ReadOnly));
}

#[test]
fn accessors_expose_construction_inputs() {
    let client = IndexerClient::new("http://example.invalid/api/v3/graphql").unwrap();
    assert_eq!(client.graphql_url(), "http://example.invalid/api/v3/graphql");
    let backend = IndexerBackend::new(client, "00aabb");
    assert_eq!(backend.address_hex(), "00aabb");
}
