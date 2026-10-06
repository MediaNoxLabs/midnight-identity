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

//! HTTP DID resolution service for `did:midnight` — the Rust counterpart
//! of [midnightntwrk/midnight-did-resolver](https://github.com/midnightntwrk/midnight-did-resolver)
//! (issue #5), built on the indexer-backed read path shipped in issue #4.
//!
//! Routes (same surface as the TS service):
//! - `GET /resolve/:did` (query params `indexerUrl`, `indexerWsUrl`)
//! - `POST /resolve` (body `{did, indexerUrl?, indexerWsUrl?}`)
//! - `GET /health`, `GET /ready`
//!
//! Responses use the W3C DID Resolution envelope
//! (`didDocument` / `didDocumentMetadata` / `didResolutionMetadata`),
//! with the TS service's error codes and status mapping:
//! `notFound` → 404, `invalidDid`/`networkMismatch` → 400,
//! `internalError` → 500. Unlike the TS service (which classifies by
//! error-message string matching), errors here are typed end-to-end.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

pub mod app;
pub mod config;
pub mod endpoint_policy;
pub mod error;
pub mod service;

pub use app::build_router;
pub use config::ResolverConfig;
pub use error::ResolutionErrorCode;
pub use service::ResolverService;
