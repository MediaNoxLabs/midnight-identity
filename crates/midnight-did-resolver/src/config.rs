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

//! Environment-driven configuration, mirroring the TS service's env
//! surface (`did-resolver-service/src/config.ts`).

use midnight_did_method::midnight_did::MidnightNetwork;

/// Service configuration. Defaults match the TS resolver.
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    /// Bind host (`RESOLVER_HOST`, default `127.0.0.1`).
    pub host: String,
    /// Bind port (`RESOLVER_PORT`, default `3001`).
    pub port: u16,
    /// Default indexer GraphQL URL (`MIDNIGHT_INDEXER_HTTP_URL`,
    /// default `http://127.0.0.1:8088/api/v3/graphql`).
    pub indexer_url: String,
    /// Optional expected network (`MIDNIGHT_NETWORK`); when set, DIDs
    /// for any other network resolve to `networkMismatch`.
    pub expected_network: Option<MidnightNetwork>,
    /// Per-resolve timeout in milliseconds (`RESOLVER_TIMEOUT_MS`,
    /// default 15000).
    pub timeout_ms: u64,
    /// Allow per-request indexer overrides targeting private/loopback
    /// hosts (`RESOLVER_ALLOW_PRIVATE_INDEXER`, default **true** for
    /// standalone/dev use — the TS service defaults to rejecting them;
    /// set to `false` for public deployments).
    pub allow_private_indexer_overrides: bool,
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 3001,
            indexer_url: "http://127.0.0.1:8088/api/v3/graphql".into(),
            expected_network: None,
            timeout_ms: 15_000,
            allow_private_indexer_overrides: true,
        }
    }
}

impl ResolverConfig {
    /// Load from the environment (unset vars keep defaults).
    ///
    /// # Errors
    ///
    /// Returns a human-readable message for unparsable values —
    /// misconfiguration should stop startup, not degrade silently.
    pub fn from_env() -> Result<Self, String> {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("RESOLVER_HOST") {
            cfg.host = v;
        }
        if let Ok(v) = std::env::var("RESOLVER_PORT") {
            cfg.port = v.parse().map_err(|e| format!("RESOLVER_PORT: {e}"))?;
        }
        if let Ok(v) = std::env::var("MIDNIGHT_INDEXER_HTTP_URL") {
            cfg.indexer_url = v;
        }
        if let Ok(v) = std::env::var("MIDNIGHT_NETWORK") {
            cfg.expected_network = Some(parse_network(&v)?);
        }
        if let Ok(v) = std::env::var("RESOLVER_TIMEOUT_MS") {
            cfg.timeout_ms = v.parse().map_err(|e| format!("RESOLVER_TIMEOUT_MS: {e}"))?;
        }
        if let Ok(v) = std::env::var("RESOLVER_ALLOW_PRIVATE_INDEXER") {
            cfg.allow_private_indexer_overrides = match v.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => true,
                "0" | "false" | "no" => false,
                other => {
                    return Err(format!(
                        "RESOLVER_ALLOW_PRIVATE_INDEXER: expected true/false/1/0/yes/no, got '{other}'"
                    ));
                }
            };
        }
        Ok(cfg)
    }
}

fn parse_network(s: &str) -> Result<MidnightNetwork, String> {
    MidnightNetwork::from_wire_str(s).ok_or_else(|| format!("MIDNIGHT_NETWORK: unknown network '{s}'"))
}
