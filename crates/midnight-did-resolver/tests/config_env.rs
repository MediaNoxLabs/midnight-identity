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

//! `ResolverConfig::from_env` parsing tests.
//!
//! Lives in an integration test (not `src/config.rs`) because the lib
//! carries `#![forbid(unsafe_code)]` and mutating the process
//! environment is `unsafe` in edition 2024. ALL env mutation is kept in
//! ONE `#[test]` so parallel tests in this binary can never race on the
//! process environment.

use midnight_did_method::midnight_did::MidnightNetwork;
use midnight_did_resolver::ResolverConfig;

const VARS: &[&str] = &[
    "RESOLVER_HOST",
    "RESOLVER_PORT",
    "MIDNIGHT_INDEXER_HTTP_URL",
    "MIDNIGHT_NETWORK",
    "RESOLVER_TIMEOUT_MS",
    "RESOLVER_ALLOW_PRIVATE_INDEXER",
];

fn clear_all() {
    for var in VARS {
        // SAFETY: single-threaded within this one test; no other test in
        // this binary touches the environment.
        unsafe { std::env::remove_var(var) };
    }
}

fn set(var: &str, value: &str) {
    // SAFETY: see `clear_all`.
    unsafe { std::env::set_var(var, value) };
}

#[test]
fn from_env_covers_defaults_overrides_and_errors() {
    // ── unset vars keep the documented defaults ──────────────────────
    clear_all();
    let cfg = ResolverConfig::from_env().expect("empty env is valid");
    assert_eq!(cfg.host, "127.0.0.1");
    assert_eq!(cfg.port, 3001);
    assert_eq!(cfg.indexer_url, "http://127.0.0.1:8088/api/v3/graphql");
    assert_eq!(cfg.expected_network, None);
    assert_eq!(cfg.timeout_ms, 15_000);
    assert!(cfg.allow_private_indexer_overrides);

    // ── every var set to a valid value ───────────────────────────────
    set("RESOLVER_HOST", "0.0.0.0");
    set("RESOLVER_PORT", "8080");
    set("MIDNIGHT_INDEXER_HTTP_URL", "https://indexer.example/api/v3/graphql");
    set("MIDNIGHT_NETWORK", "testnet");
    set("RESOLVER_TIMEOUT_MS", "2500");
    set("RESOLVER_ALLOW_PRIVATE_INDEXER", "false");
    let cfg = ResolverConfig::from_env().expect("valid overrides parse");
    assert_eq!(cfg.host, "0.0.0.0");
    assert_eq!(cfg.port, 8080);
    assert_eq!(cfg.indexer_url, "https://indexer.example/api/v3/graphql");
    assert_eq!(cfg.expected_network, Some(MidnightNetwork::Testnet));
    assert_eq!(cfg.timeout_ms, 2500);
    assert!(!cfg.allow_private_indexer_overrides);

    // ── boolean spellings (exact-match: "1"/"true"/"yes" only) ───────
    clear_all();
    for (raw, expected) in [
        ("1", true),
        ("true", true),
        ("yes", true),
        ("0", false),
        ("false", false),
        ("no", false),
        ("TRUE", false), // spelling is case-sensitive
        ("", false),
    ] {
        set("RESOLVER_ALLOW_PRIVATE_INDEXER", raw);
        let cfg = ResolverConfig::from_env().expect("bool var never errors");
        assert_eq!(
            cfg.allow_private_indexer_overrides, expected,
            "RESOLVER_ALLOW_PRIVATE_INDEXER={raw:?}"
        );
    }

    // ── unparsable values fail startup with the var named ────────────
    clear_all();
    set("RESOLVER_PORT", "not-a-port");
    let err = ResolverConfig::from_env().expect_err("bad port must fail");
    assert!(err.contains("RESOLVER_PORT"), "got {err}");

    clear_all();
    set("RESOLVER_PORT", "70000"); // out of u16 range
    let err = ResolverConfig::from_env().expect_err("oversized port must fail");
    assert!(err.contains("RESOLVER_PORT"), "got {err}");

    clear_all();
    set("RESOLVER_TIMEOUT_MS", "soon");
    let err = ResolverConfig::from_env().expect_err("bad timeout must fail");
    assert!(err.contains("RESOLVER_TIMEOUT_MS"), "got {err}");

    clear_all();
    set("MIDNIGHT_NETWORK", "narnia");
    let err = ResolverConfig::from_env().expect_err("unknown network must fail");
    assert!(err.contains("unknown network 'narnia'"), "got {err}");

    clear_all();
}
