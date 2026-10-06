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

//! Integration tests for the ledger read/write shaping helpers
//! (`ledger_utils.rs`): bound fragment-id normalization, the ledger
//! encodings of the polymorphic service `type` / `serviceEndpoint`
//! properties, and the absolute-URI assertion. Error branches are
//! pinned alongside the happy paths so the on-chain string flavour
//! stays byte-compatible with the TS port.

use midnight_did_domain::did_document::{ServiceEndpoint, ServiceEndpointArrayEntry, ServiceType};
use midnight_did_domain::ledger_utils::{
    BoundIdField, LedgerUtilsError, assert_absolute_uri, normalize_bound_fragment_id, normalize_fragment_id,
    service_endpoint_to_ledger, service_type_to_ledger,
};

const DID: &str = "did:midnight:devnet:abcd";

// ---- normalize_fragment_id -------------------------------------------

#[test]
fn fragment_id_trims_whitespace_before_normalising() {
    assert_eq!(normalize_fragment_id("  key-1  "), "#key-1");
    assert_eq!(normalize_fragment_id(" #key-1 "), "#key-1");
}

#[test]
fn fragment_id_reduces_embedded_hash_to_fragment() {
    assert_eq!(normalize_fragment_id("service#endpoint-1"), "#endpoint-1");
}

// ---- normalize_bound_fragment_id: error branches ----------------------

#[test]
fn bound_fragment_rejects_empty_input() {
    let err = normalize_bound_fragment_id("   ", BoundIdField::ServiceId, DID).unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::Empty {
            field: "service.id".into()
        }
    );
    assert_eq!(format!("{err}"), "service.id must not be empty");
}

#[test]
fn bound_fragment_rejects_network_path_reference() {
    let err = normalize_bound_fragment_id("//host/path", BoundIdField::MethodId, DID).unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::NotARelativeOrDidUrl {
            field: "methodId".into()
        }
    );
}

#[test]
fn bound_fragment_rejects_did_url_without_fragment() {
    let err = normalize_bound_fragment_id(DID, BoundIdField::VerificationMethodId, DID).unwrap_err();
    assert!(matches!(err, LedgerUtilsError::EmptyFragment { .. }));
    assert_eq!(
        format!("{err}"),
        "verificationMethod.id DID URL must include a non-empty fragment identifier"
    );
}

#[test]
fn bound_fragment_rejects_did_url_with_trailing_empty_fragment() {
    let err = normalize_bound_fragment_id(&format!("{DID}#"), BoundIdField::VerificationMethodId, DID).unwrap_err();
    assert!(matches!(err, LedgerUtilsError::EmptyFragment { .. }));
}

#[test]
fn bound_fragment_rejects_subject_mismatch_with_expected_did_in_message() {
    let err = normalize_bound_fragment_id(
        "did:midnight:devnet:zzzz#key-1",
        BoundIdField::SchnorrJubjubVerificationMethodId,
        DID,
    )
    .unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::SubjectMismatch {
            field: "schnorrJubjubVerificationMethod.id".into(),
            expected: DID.into(),
        }
    );
    assert_eq!(
        format!("{err}"),
        format!("schnorrJubjubVerificationMethod.id DID URL subject must match the current DID ({DID})")
    );
}

#[test]
fn bound_fragment_rejects_absolute_uri_scheme() {
    let err = normalize_bound_fragment_id("https://example.com/x", BoundIdField::ShortServiceId, DID).unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::NotARelativeOrDidUrl {
            field: "serviceId".into()
        }
    );
    assert_eq!(format!("{err}"), "serviceId must be a DID URL or relative reference");
}

// ---- normalize_bound_fragment_id: accepted shapes ----------------------

#[test]
fn bound_fragment_passes_through_leading_hash() {
    let out = normalize_bound_fragment_id("#key-1", BoundIdField::VerificationMethodId, DID).unwrap();
    assert_eq!(out, "#key-1");
}

#[test]
fn bound_fragment_reduces_matching_did_url() {
    let out = normalize_bound_fragment_id(&format!("{DID}#auth-key"), BoundIdField::MethodId, DID).unwrap();
    assert_eq!(out, "#auth-key");
}

#[test]
fn bound_fragment_prefixes_path_like_relative_references() {
    assert_eq!(
        normalize_bound_fragment_id("/path", BoundIdField::ServiceId, DID).unwrap(),
        "#/path"
    );
    assert_eq!(
        normalize_bound_fragment_id("./rel", BoundIdField::ServiceId, DID).unwrap(),
        "#./rel"
    );
    assert_eq!(
        normalize_bound_fragment_id("?query", BoundIdField::ServiceId, DID).unwrap(),
        "#?query"
    );
}

#[test]
fn bound_fragment_prefixes_bare_names() {
    assert_eq!(
        normalize_bound_fragment_id("key-1", BoundIdField::ServiceId, DID).unwrap(),
        "#key-1"
    );
}

// ---- service_type_to_ledger -------------------------------------------

#[test]
fn service_type_single_is_trimmed_and_passed_through() {
    let out = service_type_to_ledger(&ServiceType::One("  LinkedDomains  ".into())).unwrap();
    assert_eq!(out, "LinkedDomains");
}

#[test]
fn service_type_single_rejects_blank() {
    let err = service_type_to_ledger(&ServiceType::One("   ".into())).unwrap_err();
    assert_eq!(err, LedgerUtilsError::EmptyServiceType);
    assert_eq!(format!("{err}"), "service type must not be empty");
}

#[test]
fn service_type_rejects_empty_array() {
    let err = service_type_to_ledger(&ServiceType::Many(vec![])).unwrap_err();
    assert_eq!(err, LedgerUtilsError::InvalidServiceType);
    assert_eq!(format!("{err}"), "service type property must be a non-empty string set");
}

#[test]
fn service_type_rejects_blank_entry_in_array() {
    let err = service_type_to_ledger(&ServiceType::Many(vec!["A".into(), "  ".into()])).unwrap_err();
    assert_eq!(err, LedgerUtilsError::EmptyServiceTypeEntries);
    assert_eq!(format!("{err}"), "service type entries must not be empty");
}

#[test]
fn service_type_rejects_duplicate_entries_after_trimming() {
    let err = service_type_to_ledger(&ServiceType::Many(vec!["A".into(), " A ".into()])).unwrap_err();
    assert_eq!(err, LedgerUtilsError::DuplicateServiceTypeEntries);
    assert_eq!(format!("{err}"), "service type entries must be unique");
}

#[test]
fn service_type_singleton_array_collapses_to_bare_string() {
    let out = service_type_to_ledger(&ServiceType::Many(vec![" LinkedDomains ".into()])).unwrap();
    assert_eq!(out, "LinkedDomains");
}

#[test]
fn service_type_multi_array_is_json_encoded() {
    let out = service_type_to_ledger(&ServiceType::Many(vec!["A".into(), "B".into()])).unwrap();
    assert_eq!(out, r#"["A","B"]"#);
    // The encoding must round-trip back to the original list.
    let back: Vec<String> = serde_json::from_str(&out).unwrap();
    assert_eq!(back, vec!["A".to_string(), "B".to_string()]);
}

// ---- service_endpoint_to_ledger -----------------------------------------

#[test]
fn endpoint_uri_is_normalized_then_json_encoded() {
    let out = service_endpoint_to_ledger(ServiceEndpoint::Uri("HTTPS://Example.com:443/".into()));
    assert_eq!(out, r#""https://example.com/""#);
}

#[test]
fn endpoint_object_is_normalized_recursively() {
    let mut map = serde_json::Map::new();
    map.insert("uri".into(), serde_json::json!("HTTP://A.example:80/"));
    let out = service_endpoint_to_ledger(ServiceEndpoint::Object(map));
    assert_eq!(out, r#"{"uri":"http://a.example/"}"#);
}

#[test]
fn endpoint_array_mixes_uris_and_objects() {
    let mut map = serde_json::Map::new();
    map.insert("uri".into(), serde_json::json!("WSS://B.example:443/"));
    let out = service_endpoint_to_ledger(ServiceEndpoint::Array(vec![
        ServiceEndpointArrayEntry::Uri("HTTPS://A.example/".into()),
        ServiceEndpointArrayEntry::Object(map),
    ]));
    assert_eq!(out, r#"["https://a.example/",{"uri":"wss://b.example/"}]"#);
    // Round-trip: ledger string decodes back into a ServiceEndpoint array.
    let back: ServiceEndpoint = serde_json::from_str(&out).unwrap();
    assert!(matches!(back, ServiceEndpoint::Array(entries) if entries.len() == 2));
}

// ---- assert_absolute_uri --------------------------------------------------

#[test]
fn absolute_uri_accepts_and_trims_valid_uri() {
    let out = assert_absolute_uri("  https://example.com/path  ", None).unwrap();
    assert_eq!(out, "https://example.com/path");
}

#[test]
fn absolute_uri_accepts_non_network_schemes() {
    assert_eq!(
        assert_absolute_uri("did:example:1234", None).unwrap(),
        "did:example:1234"
    );
}

#[test]
fn absolute_uri_rejects_empty_with_default_label() {
    let err = assert_absolute_uri("   ", None).unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::Empty {
            field: "aliasUri".into()
        }
    );
}

#[test]
fn absolute_uri_rejects_relative_reference_with_custom_label() {
    let err = assert_absolute_uri("just/a/path", Some("alsoKnownAs")).unwrap_err();
    assert_eq!(
        err,
        LedgerUtilsError::NotAbsoluteUri {
            field: "alsoKnownAs".into()
        }
    );
    assert_eq!(format!("{err}"), "alsoKnownAs must be a valid absolute URI (RFC3986)");
}
