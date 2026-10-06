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

//! End-to-end route tests against a mock indexer serving REAL captured
//! chain state (`tests/fixtures/state-050-enriched.hex` — a 0.5.0 DID
//! deployed on the standalone stack with a JWK verification method and
//! an authentication relation; address in the sibling `.addr` file).

use std::sync::Arc;

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use midnight_did_resolver::{ResolverConfig, ResolverService, build_router};
use serde_json::{Value, json};

const STATE_HEX: &str = include_str!("fixtures/state-050-enriched.hex");
const ADDR: &str = include_str!("fixtures/state-050-enriched.addr");

fn fixture_addr() -> String {
    ADDR.trim().to_string()
}

/// Serve a GraphQL endpoint that returns the fixture state for the
/// fixture address and `null` for everything else.
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

async fn spawn_resolver(indexer_url: String) -> String {
    let config = ResolverConfig {
        indexer_url,
        ..ResolverConfig::default()
    };
    let router = build_router(Arc::new(ResolverService::new(config)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://127.0.0.1:{port}")
}

async fn setup() -> String {
    spawn_resolver(spawn_mock_indexer().await).await
}

#[tokio::test]
async fn get_resolve_returns_full_envelope() {
    let base = setup().await;
    let did = format!("did:midnight:undeployed:{}", fixture_addr());
    let resp = reqwest::get(format!("{base}/resolve/{did}")).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();

    assert_eq!(body["didResolutionMetadata"]["contentType"], "application/did+ld+json");
    assert_eq!(body["didResolutionMetadata"]["error"], Value::Null);
    assert_eq!(body["didDocument"]["id"], did.as_str());
    // The enriched fixture carries #key-1 + an authentication relation.
    assert_eq!(body["didDocument"]["authentication"][0], "#key-1");
    assert_eq!(
        body["didDocument"]["verificationMethod"][0]["publicKeyJwk"]["kty"],
        "OKP"
    );
    assert_eq!(body["didDocumentMetadata"]["versionId"], "2");
    assert!(body["didDocumentMetadata"]["created"].is_string());
}

#[tokio::test]
async fn post_resolve_matches_get() {
    let base = setup().await;
    let did = format!("did:midnight:undeployed:{}", fixture_addr());
    let client = reqwest::Client::new();
    let get_body: Value = client
        .get(format!("{base}/resolve/{did}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let post_body: Value = client
        .post(format!("{base}/resolve"))
        .json(&json!({"did": did}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(get_body, post_body);
}

#[tokio::test]
async fn unknown_address_is_not_found_404() {
    let base = setup().await;
    let did = "did:midnight:undeployed:1111111111111111111111111111111111111111111111111111111111111111";
    let resp = reqwest::get(format!("{base}/resolve/{did}")).await.unwrap();
    assert_eq!(resp.status(), 404);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["didResolutionMetadata"]["error"], "notFound");
    assert_eq!(body["didDocument"], Value::Null);
}

#[tokio::test]
async fn invalid_did_is_400() {
    let base = setup().await;
    let resp = reqwest::get(format!("{base}/resolve/not-a-did")).await.unwrap();
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["didResolutionMetadata"]["error"], "invalidDid");
}

#[tokio::test]
async fn network_mismatch_is_400() {
    let mock = spawn_mock_indexer().await;
    let config = ResolverConfig {
        indexer_url: mock,
        expected_network: Some(midnight_did_method::midnight_did::MidnightNetwork::Testnet),
        ..ResolverConfig::default()
    };
    let router = build_router(Arc::new(ResolverService::new(config)));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let did = format!("did:midnight:undeployed:{}", fixture_addr());
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/resolve/{did}"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["didResolutionMetadata"]["error"], "networkMismatch");
}

#[tokio::test]
async fn indexer_down_is_internal_error_500() {
    // Point at a port with nothing listening.
    let base = spawn_resolver("http://127.0.0.1:1/api/v3/graphql".into()).await;
    let did = format!("did:midnight:undeployed:{}", fixture_addr());
    let resp = reqwest::get(format!("{base}/resolve/{did}")).await.unwrap();
    assert_eq!(resp.status(), 500);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["didResolutionMetadata"]["error"], "internalError");
}

#[tokio::test]
async fn health_and_ready() {
    let base = setup().await;
    for (path, expected) in [("health", "ok"), ("ready", "ready")] {
        let body: Value = reqwest::get(format!("{base}/{path}"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["status"], expected);
    }
}
