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

//! `ResolverService::resolve` branch tests the HTTP route tests don't
//! reach: the offchain-DID guard, indexer-override policy outcomes, and
//! the resolve timeout — against the same mock indexer + captured-state
//! fixture as `routes.rs`.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use midnight_did_resolver::error::ResolutionErrorCode;
use midnight_did_resolver::{ResolverConfig, ResolverService};
use serde_json::{Value, json};

const STATE_HEX: &str = include_str!("fixtures/state-050-enriched.hex");
const ADDR: &str = include_str!("fixtures/state-050-enriched.addr");

fn fixture_addr() -> String {
    ADDR.trim().to_string()
}

fn fixture_did() -> String {
    format!("did:midnight:undeployed:{}", fixture_addr())
}

/// Serve a GraphQL endpoint that returns the fixture state for the
/// fixture address and `null` for everything else (same shape as the
/// `routes.rs` mock).
async fn spawn_mock_indexer() -> String {
    async fn graphql(State(addr): State<String>, Json(body): Json<Value>) -> Json<Value> {
        let requested = body["variables"]["address"].as_str().unwrap_or_default();
        if requested.eq_ignore_ascii_case(&addr) {
            Json(json!({"data": {"contractAction": {"state": STATE_HEX.trim()}}}))
        } else {
            Json(json!({"data": {"contractAction": null}}))
        }
    }
    let router = Router::new()
        .route("/api/v3/graphql", post(graphql))
        .with_state(fixture_addr());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://127.0.0.1:{port}/api/v3/graphql")
}

fn service_with(config: ResolverConfig) -> ResolverService {
    ResolverService::new(config)
}

#[tokio::test]
async fn offchain_did_is_network_mismatch() {
    // Offchain DIDs never reach the indexer, so a dead default URL is fine.
    let service = service_with(ResolverConfig {
        indexer_url: "http://127.0.0.1:1/api/v3/graphql".into(),
        ..ResolverConfig::default()
    });
    let did = format!("did:midnight:offchain:{}", "ab".repeat(32));
    let err = service.resolve(&did, None).await.unwrap_err();
    assert_eq!(err.code, ResolutionErrorCode::NetworkMismatch);
    assert!(err.detail.contains("offchain"), "got {}", err.detail);
}

#[tokio::test]
async fn private_override_is_rejected_as_invalid_did_when_disallowed() {
    let mock = spawn_mock_indexer().await;
    let service = service_with(ResolverConfig {
        indexer_url: mock.clone(),
        allow_private_indexer_overrides: false,
        ..ResolverConfig::default()
    });
    // The override targets loopback — the SSRF policy must reject it
    // even though the default endpoint (same host!) stays exempt.
    let err = service.resolve(&fixture_did(), Some(&mock)).await.unwrap_err();
    assert_eq!(err.code, ResolutionErrorCode::InvalidDid);
    assert!(err.detail.contains("private/loopback"), "got {}", err.detail);

    // No override → the configured default is used and resolution works.
    let resolved = service.resolve(&fixture_did(), None).await.expect("default resolves");
    assert_eq!(resolved.did_document.id.as_str(), fixture_did());
}

#[tokio::test]
async fn bad_scheme_override_is_invalid_did() {
    let service = service_with(ResolverConfig::default());
    let err = service
        .resolve(&fixture_did(), Some("ftp://indexer.example/graphql"))
        .await
        .unwrap_err();
    assert_eq!(err.code, ResolutionErrorCode::InvalidDid);
    assert!(err.detail.contains("scheme"), "got {}", err.detail);
}

#[tokio::test]
async fn accepted_override_wins_over_default_endpoint() {
    // Default endpoint is dead; only the override can serve the state.
    let mock = spawn_mock_indexer().await;
    let service = service_with(ResolverConfig {
        indexer_url: "http://127.0.0.1:1/api/v3/graphql".into(),
        allow_private_indexer_overrides: true,
        ..ResolverConfig::default()
    });
    let resolved = service
        .resolve(&fixture_did(), Some(&mock))
        .await
        .expect("override endpoint resolves");
    assert_eq!(resolved.did_document.id.as_str(), fixture_did());
    let methods = resolved.did_document.verification_method.expect("methods present");
    assert!(!methods.is_empty());
}

#[tokio::test]
async fn zero_timeout_is_internal_error() {
    let mock = spawn_mock_indexer().await;
    let service = service_with(ResolverConfig {
        indexer_url: mock,
        timeout_ms: 0,
        ..ResolverConfig::default()
    });
    let err = service.resolve(&fixture_did(), None).await.unwrap_err();
    assert_eq!(err.code, ResolutionErrorCode::InternalError);
    assert!(err.detail.contains("timed out"), "got {}", err.detail);
}
