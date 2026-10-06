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

//! axum router: the TS service's HTTP surface.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::error::ResolutionError;
use crate::service::ResolverService;

/// Query parameters accepted by `GET /resolve/{did}`.
#[derive(Debug, Deserialize)]
pub struct ResolveQuery {
    /// Per-request indexer GraphQL URL override.
    #[serde(rename = "indexerUrl")]
    indexer_url: Option<String>,
}

/// Body accepted by `POST /resolve`.
#[derive(Debug, Deserialize)]
pub struct ResolveBody {
    /// The DID to resolve.
    did: String,
    /// Per-request indexer GraphQL URL override.
    #[serde(rename = "indexerUrl")]
    indexer_url: Option<String>,
}

/// Build the router over a shared [`ResolverService`].
pub fn build_router(service: Arc<ResolverService>) -> Router {
    Router::new()
        .route("/resolve/{did}", get(resolve_get))
        .route("/resolve", post(resolve_post))
        .route("/health", get(|| async { Json(json!({"status": "ok"})) }))
        .route("/ready", get(|| async { Json(json!({"status": "ready"})) }))
        .with_state(service)
}

async fn resolve_get(
    State(service): State<Arc<ResolverService>>,
    Path(did): Path<String>,
    Query(query): Query<ResolveQuery>,
) -> Response {
    envelope(service.resolve(&did, query.indexer_url.as_deref()).await)
}

async fn resolve_post(State(service): State<Arc<ResolverService>>, Json(body): Json<ResolveBody>) -> Response {
    envelope(service.resolve(&body.did, body.indexer_url.as_deref()).await)
}

/// W3C DID Resolution envelope with the TS service's status mapping.
fn envelope(result: Result<midnight_did_api::resolution::ResolvedMidnightDid, ResolutionError>) -> Response {
    match result {
        Ok(resolved) => {
            let body = json!({
                "didDocument": resolved.did_document,
                "didDocumentMetadata": resolved.did_document_metadata,
                "didResolutionMetadata": {
                    "contentType": "application/did+ld+json",
                    "error": null,
                },
            });
            (StatusCode::OK, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
        }
        Err(err) => {
            tracing::warn!(code = ?err.code, detail = %err.detail, "resolution failed");
            let status = StatusCode::from_u16(err.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let body = json!({
                "didDocument": null,
                "didDocumentMetadata": {},
                "didResolutionMetadata": {
                    "contentType": null,
                    "error": err.code,
                },
            });
            (status, [(header::CONTENT_TYPE, "application/json")], Json(body)).into_response()
        }
    }
}
