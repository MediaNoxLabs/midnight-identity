// This file is part of Compact.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//  	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Integration tests for RFC 3986 URI normalization (`uri.rs`).
//!
//! Pins the scheme/host case-folding, default-port removal, userinfo
//! and query/fragment preservation rules, the best-effort passthrough
//! behaviour for unknown or unparseable inputs, and the recursive
//! JSON walker `normalize_service_endpoint_value`.

use midnight_did_domain::uri::{normalize_service_endpoint_value, normalize_uri_string};
use serde_json::json;

// ---- Scheme + host case folding ------------------------------------

#[test]
fn lowercases_mixed_case_scheme_and_host_for_ws() {
    assert_eq!(normalize_uri_string("WS://Example.COM/Chat"), "ws://example.com/Chat");
}

#[test]
fn lowercases_mixed_case_scheme_and_host_for_wss() {
    assert_eq!(normalize_uri_string("WSS://EXAMPLE.com/x"), "wss://example.com/x");
}

// ---- Default and non-default ports ----------------------------------

#[test]
fn strips_default_port_for_http() {
    assert_eq!(normalize_uri_string("http://example.com:80/"), "http://example.com/");
}

#[test]
fn strips_default_port_for_ws() {
    assert_eq!(normalize_uri_string("ws://example.com:80/"), "ws://example.com/");
}

#[test]
fn strips_default_port_for_wss() {
    assert_eq!(normalize_uri_string("wss://example.com:443/"), "wss://example.com/");
}

#[test]
fn keeps_non_default_port() {
    assert_eq!(
        normalize_uri_string("https://example.com:8443/api"),
        "https://example.com:8443/api"
    );
}

#[test]
fn strips_mismatched_scheme_default_pairs_only_when_matching() {
    // 443 is not the default port for plain http, so it must survive.
    assert_eq!(
        normalize_uri_string("http://example.com:443/"),
        "http://example.com:443/"
    );
}

// ---- Path handling ---------------------------------------------------

#[test]
fn drops_synthetic_root_path_when_input_had_no_trailing_slash() {
    assert_eq!(normalize_uri_string("https://Example.com"), "https://example.com");
}

#[test]
fn keeps_explicit_trailing_slash() {
    assert_eq!(normalize_uri_string("https://example.com/"), "https://example.com/");
}

// ---- Userinfo, query, fragment --------------------------------------

#[test]
fn preserves_username_without_password() {
    assert_eq!(
        normalize_uri_string("https://alice@Example.com/inbox"),
        "https://alice@example.com/inbox"
    );
}

#[test]
fn preserves_username_and_password() {
    assert_eq!(
        normalize_uri_string("https://alice:secret@Example.com/"),
        "https://alice:secret@example.com/"
    );
}

#[test]
fn preserves_query_and_fragment() {
    assert_eq!(
        normalize_uri_string("HTTPS://Example.com/p?a=1&b=2#Frag"),
        "https://example.com/p?a=1&b=2#Frag"
    );
}

// ---- Passthrough behaviour -------------------------------------------

#[test]
fn leaves_unknown_scheme_with_authority_alone() {
    assert_eq!(
        normalize_uri_string("ftp://Example.COM:21/file"),
        "ftp://Example.COM:21/file"
    );
}

#[test]
fn leaves_mailto_alone() {
    assert_eq!(normalize_uri_string("mailto:Bob@Example.com"), "mailto:Bob@Example.com");
}

#[test]
fn leaves_empty_string_alone() {
    assert_eq!(normalize_uri_string(""), "");
}

#[test]
fn leaves_relative_reference_alone() {
    assert_eq!(normalize_uri_string("path/to/resource"), "path/to/resource");
}

#[test]
fn leaves_value_with_digit_scheme_prefix_alone() {
    // A scheme must start with a letter — this has none, so no rewrite.
    assert_eq!(normalize_uri_string("1http://Example.com"), "1http://Example.com");
}

#[test]
fn leaves_value_with_invalid_scheme_char_alone() {
    assert_eq!(normalize_uri_string("ht~tp://Example.com"), "ht~tp://Example.com");
}

#[test]
fn leaves_unparseable_known_scheme_value_alone() {
    // Known scheme but the URL parser rejects it (empty host).
    assert_eq!(normalize_uri_string("http://"), "http://");
}

// ---- normalize_service_endpoint_value --------------------------------

#[test]
fn walker_normalizes_plain_string() {
    let out = normalize_service_endpoint_value(json!("HTTPS://Example.com/"));
    assert_eq!(out, json!("https://example.com/"));
}

#[test]
fn walker_normalizes_strings_inside_arrays() {
    let out = normalize_service_endpoint_value(json!(["HTTP://A.example/", "did:example:1"]));
    assert_eq!(out, json!(["http://a.example/", "did:example:1"]));
}

#[test]
fn walker_normalizes_strings_inside_nested_objects() {
    let out = normalize_service_endpoint_value(json!({
        "uri": "WSS://Node.Example:443/",
        "routingKeys": ["HTTPS://Relay.Example/"],
        "nested": { "deep": "HTTP://Deep.Example:80/" }
    }));
    assert_eq!(
        out,
        json!({
            "uri": "wss://node.example/",
            "routingKeys": ["https://relay.example/"],
            "nested": { "deep": "http://deep.example/" }
        })
    );
}

#[test]
fn walker_leaves_non_string_scalars_alone() {
    let value = json!({ "n": 42, "b": true, "nil": null });
    assert_eq!(normalize_service_endpoint_value(value.clone()), value);
}
