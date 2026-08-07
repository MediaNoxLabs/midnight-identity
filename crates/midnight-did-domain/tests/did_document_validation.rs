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

//! Integration tests for the W3C DID Core data model (`did_document.rs`)
//! beyond the constructor gates covered in `constructors.rs`:
//!
//! - DID string newtype parsing (`DidUrl`, `RelativeUrl`, `DidString`,
//!   `DidKeyId`) including reject paths and accessor round-trips.
//! - serde wire spellings for the enum vocabulary (`"P-256"`,
//!   `"secp256k1"`, media types, camelCase resolution error codes).
//! - `PublicKeyJwk` coordinate-length table and per-issue validation
//!   branches.
//! - `DidDocument::validate` cross-consistency issues (duplicate ids,
//!   dangling relations, duplicate service endpoints) with their
//!   dot-joined issue paths.
//! - `parse_*` helpers, metadata/resolution envelopes, and
//!   `create_did_document` / builder edges not exercised elsewhere.

use std::collections::BTreeMap;

use midnight_did_domain::did_document::{
    Controller, CreateDidDocumentParams, CurveType, DidDocument, DidDocumentBuilder, DidDocumentMetadata, DidKeyId,
    DidResolutionErrorCode, DidResolutionMetadata, DidResolutionResult, DidString, DidUrl, DocumentContext, KeyType,
    KnownDidMediaType, KnownDidResolutionErrorCode, NewPublicKeyJwk, NewService, NewVerificationMethod, PublicKeyJwk,
    PublicKeyJwkCoordinate, RelativeUrl, Service, ServiceEndpoint, ServiceEndpointArrayEntry, ServiceType,
    ValidationError, ValidationIssue, VerificationMethod, VerificationMethodRelation, VerificationMethodType,
    create_did_document, normalize_service_endpoint, parse_did, parse_did_document, parse_did_key_id, parse_did_url,
    parse_service, parse_verification_method, public_key_jwk_coordinate_byte_length,
};
use serde_json::json;

const SUBJECT: &str = "did:midnight:testnet:abc";
// 43-char unpadded base64url == 32 canonical bytes.
const COORD_32: &str = "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo";

fn ed25519_jwk() -> PublicKeyJwk {
    PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::OKP,
        crv: CurveType::Ed25519,
        x: COORD_32.to_string(),
        y: None,
        extensions: BTreeMap::new(),
    })
    .expect("valid JWK")
}

fn vm(id: &str) -> VerificationMethod {
    VerificationMethod::new(NewVerificationMethod {
        id: id.to_string(),
        type_: VerificationMethodType::JsonWebKey,
        controller: SUBJECT.to_string(),
        public_key_jwk: ed25519_jwk(),
    })
    .expect("valid VM")
}

fn service(id: &str, endpoint: ServiceEndpoint) -> Service {
    Service::new(NewService {
        id: id.to_string(),
        type_: ServiceType::One("LinkedDomains".to_string()),
        service_endpoint: endpoint,
    })
    .expect("valid service")
}

fn minimal_doc() -> DidDocument {
    DidDocumentBuilder::new(SUBJECT).build().expect("minimal doc")
}

// ---- ValidationIssue / ValidationError --------------------------------

#[test]
fn validation_error_summary_appends_dot_joined_paths() {
    let err = ValidationError::from_issues(vec![
        ValidationIssue::new("top-level problem"),
        ValidationIssue::at("nested problem", vec!["service".into(), "0".into(), "id".into()]),
    ]);
    assert_eq!(err.summary, "top-level problem; nested problem at service.0.id");
    assert_eq!(err.issues.len(), 2);
    assert_eq!(format!("{err}"), format!("validation failed: {}", err.summary));
}

// ---- DID string newtypes ----------------------------------------------

#[test]
fn did_url_accepts_paths_queries_and_fragments() {
    let url = DidUrl::parse("did:example:123/path?query=1#frag").expect("valid DID URL");
    assert_eq!(url.as_str(), "did:example:123/path?query=1#frag");
    assert_eq!(url.into_string(), "did:example:123/path?query=1#frag");
}

#[test]
fn did_url_rejects_non_did_values() {
    assert!(DidUrl::parse("http://example.com").is_err());
    assert!(DidUrl::parse("did:").is_err());
    assert!(DidUrl::parse("did:example").is_err());
    let err = DidUrl::parse("").unwrap_err();
    assert!(err.summary.contains("Invalid DID URL format"));
}

#[test]
fn relative_url_accepts_paths_and_fragments() {
    let rel = RelativeUrl::parse("path/to/resource").expect("valid relative URL");
    assert_eq!(rel.as_str(), "path/to/resource");
    assert_eq!(rel.into_string(), "path/to/resource");
    assert!(RelativeUrl::parse("#frag").is_ok());
    assert!(RelativeUrl::parse("?query").is_ok());
}

#[test]
fn relative_url_rejects_schemes_network_paths_and_padding() {
    assert!(RelativeUrl::parse("https://example.com").is_err());
    assert!(RelativeUrl::parse("//host/path").is_err());
    assert!(RelativeUrl::parse("").is_err());
    let err = RelativeUrl::parse(" padded ").unwrap_err();
    assert!(err.summary.contains("Relative URL must be relative to the DID subject"));
}

#[test]
fn did_string_accessors_round_trip() {
    let did = DidString::parse(SUBJECT).unwrap();
    assert_eq!(did.as_str(), SUBJECT);
    assert_eq!(did.into_string(), SUBJECT);
}

#[test]
fn did_key_id_accessors_round_trip() {
    let id = DidKeyId::parse("#key-1").unwrap();
    assert_eq!(id.as_str(), "#key-1");
    assert_eq!(id.into_string(), "#key-1");
}

#[test]
fn did_key_id_rejects_invalid_fragment_characters() {
    assert!(DidKeyId::parse("did:example:1#bad frag").is_err());
    assert!(DidKeyId::parse("#").is_err());
    let err = DidKeyId::parse("https://example.com#key").unwrap_err();
    assert!(err.summary.contains("Invalid DID Key ID format"));
}

// ---- parse_* helpers ----------------------------------------------------

#[test]
fn parse_helpers_forward_newtype_validation() {
    assert!(parse_did_url("did:example:1#f").is_ok());
    assert!(parse_did_url("nope").is_err());
    assert!(parse_did_key_id("#key-1").is_ok());
    assert!(parse_did_key_id("did:example:1").is_err());
    assert!(parse_did(SUBJECT).is_ok());
    assert!(parse_did("did:example:1#frag").is_err());
}

// ---- serde enum wire spellings ------------------------------------------

#[test]
fn curve_type_uses_ts_wire_spellings() {
    assert_eq!(serde_json::to_value(CurveType::P256).unwrap(), json!("P-256"));
    assert_eq!(serde_json::to_value(CurveType::Secp256k1).unwrap(), json!("secp256k1"));
    assert_eq!(serde_json::to_value(CurveType::Ed25519).unwrap(), json!("Ed25519"));
    assert_eq!(serde_json::to_value(CurveType::X25519).unwrap(), json!("X25519"));
    assert_eq!(serde_json::to_value(CurveType::Jubjub).unwrap(), json!("Jubjub"));
    assert_eq!(
        serde_json::to_value(CurveType::BLS12381G1).unwrap(),
        json!("BLS12381G1")
    );
    assert_eq!(
        serde_json::to_value(CurveType::BLS12381G2).unwrap(),
        json!("BLS12381G2")
    );
    // And back.
    assert_eq!(
        serde_json::from_value::<CurveType>(json!("P-256")).unwrap(),
        CurveType::P256
    );
    assert_eq!(
        serde_json::from_value::<CurveType>(json!("secp256k1")).unwrap(),
        CurveType::Secp256k1
    );
    assert!(serde_json::from_value::<CurveType>(json!("P256")).is_err());
}

#[test]
fn key_type_round_trips_all_variants() {
    for (kty, wire) in [
        (KeyType::EC, "EC"),
        (KeyType::RSA, "RSA"),
        (KeyType::oct, "oct"),
        (KeyType::OKP, "OKP"),
    ] {
        assert_eq!(serde_json::to_value(kty).unwrap(), json!(wire));
        assert_eq!(serde_json::from_value::<KeyType>(json!(wire)).unwrap(), kty);
    }
}

#[test]
fn verification_method_enums_round_trip() {
    assert_eq!(
        serde_json::to_value(VerificationMethodType::JsonWebKey).unwrap(),
        json!("JsonWebKey")
    );
    assert_eq!(
        serde_json::from_value::<VerificationMethodType>(json!("Undefined")).unwrap(),
        VerificationMethodType::Undefined
    );
    assert_eq!(
        serde_json::to_value(VerificationMethodRelation::AssertionMethod).unwrap(),
        json!("AssertionMethod")
    );
    assert_eq!(
        serde_json::from_value::<VerificationMethodRelation>(json!("KeyAgreement")).unwrap(),
        VerificationMethodRelation::KeyAgreement
    );
}

#[test]
fn known_media_types_use_mime_wire_spellings() {
    for (variant, wire) in [
        (KnownDidMediaType::DidLdJson, "application/did+ld+json"),
        (KnownDidMediaType::DidJson, "application/did+json"),
        (KnownDidMediaType::LdJson, "application/ld+json"),
        (KnownDidMediaType::Json, "application/json"),
    ] {
        assert_eq!(serde_json::to_value(variant).unwrap(), json!(wire));
        assert_eq!(
            serde_json::from_value::<KnownDidMediaType>(json!(wire)).unwrap(),
            variant
        );
    }
}

#[test]
fn known_resolution_error_codes_are_camel_case() {
    for (variant, wire) in [
        (KnownDidResolutionErrorCode::InvalidDid, "invalidDid"),
        (KnownDidResolutionErrorCode::MethodNotSupported, "methodNotSupported"),
        (
            KnownDidResolutionErrorCode::NotAllowedGlobalDuplicateKey,
            "notAllowedGlobalDuplicateKey",
        ),
        (KnownDidResolutionErrorCode::NotFound, "notFound"),
    ] {
        assert_eq!(serde_json::to_value(variant).unwrap(), json!(wire));
        assert_eq!(
            serde_json::from_value::<KnownDidResolutionErrorCode>(json!(wire)).unwrap(),
            variant
        );
    }
}

// ---- JWK coordinate-length table ------------------------------------------

#[test]
fn coordinate_byte_length_matches_ts_table() {
    use PublicKeyJwkCoordinate::{X, Y};
    // 32-byte x for the five "small" curves regardless of kty.
    for crv in [
        CurveType::Ed25519,
        CurveType::X25519,
        CurveType::Jubjub,
        CurveType::P256,
        CurveType::Secp256k1,
    ] {
        assert_eq!(public_key_jwk_coordinate_byte_length(KeyType::OKP, crv, X), Some(32));
    }
    // BLS curves are OKP-only for x.
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::OKP, CurveType::BLS12381G1, X),
        Some(48)
    );
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::OKP, CurveType::BLS12381G2, X),
        Some(96)
    );
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::EC, CurveType::BLS12381G1, X),
        None
    );
    // y is defined only for EC keys.
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::EC, CurveType::P256, Y),
        Some(32)
    );
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::OKP, CurveType::Ed25519, Y),
        None
    );
    assert_eq!(
        public_key_jwk_coordinate_byte_length(KeyType::RSA, CurveType::P256, Y),
        None
    );
}

// ---- PublicKeyJwk validation branches --------------------------------------

#[test]
fn public_key_jwk_accepts_ec_p256_with_y_and_exposes_accessors() {
    let mut extensions = BTreeMap::new();
    extensions.insert("use".to_string(), json!("sig"));
    let jwk = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::EC,
        crv: CurveType::P256,
        x: COORD_32.to_string(),
        y: Some(COORD_32.to_string()),
        extensions,
    })
    .expect("valid EC P-256 JWK");
    assert_eq!(jwk.kty(), KeyType::EC);
    assert_eq!(jwk.crv(), CurveType::P256);
    assert_eq!(jwk.x(), COORD_32);
    assert_eq!(jwk.y(), Some(COORD_32));
    assert_eq!(jwk.extensions().get("use"), Some(&json!("sig")));
    assert!(jwk.validate().is_ok());
}

#[test]
fn public_key_jwk_rejects_ec_with_okp_curve() {
    let err = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::EC,
        crv: CurveType::Ed25519,
        x: COORD_32.to_string(),
        y: Some(COORD_32.to_string()),
        extensions: BTreeMap::new(),
    })
    .unwrap_err();
    assert!(format!("{err}").contains("EC keys must use Jubjub, P-256, or secp256k1 curve"));
}

#[test]
fn public_key_jwk_rejects_oct_without_y() {
    let err = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::oct,
        crv: CurveType::P256,
        x: COORD_32.to_string(),
        y: None,
        extensions: BTreeMap::new(),
    })
    .unwrap_err();
    assert!(format!("{err}").contains("Non-OKP keys must include a y coordinate"));
}

#[test]
fn public_key_jwk_rejects_wrong_length_x() {
    let err = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::OKP,
        crv: CurveType::Ed25519,
        x: "AAAA".to_string(), // 3 bytes, not 32
        y: None,
        extensions: BTreeMap::new(),
    })
    .unwrap_err();
    assert!(format!("{err}").contains("publicKeyJwk.x must be canonical base64url"));
}

#[test]
fn public_key_jwk_rejects_wrong_length_y() {
    let err = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::EC,
        crv: CurveType::P256,
        x: COORD_32.to_string(),
        y: Some("AAAA".to_string()),
        extensions: BTreeMap::new(),
    })
    .unwrap_err();
    assert!(format!("{err}").contains("publicKeyJwk.y must be canonical base64url"));
}

#[test]
fn public_key_jwk_decode_x_returns_canonical_bytes() {
    let bytes = ed25519_jwk().decode_x().expect("decodable x");
    assert_eq!(bytes.len(), 32);
}

#[test]
fn public_key_jwk_serialize_round_trips_with_extensions() {
    let mut extensions = BTreeMap::new();
    extensions.insert("kid".to_string(), json!("key-1"));
    let jwk = PublicKeyJwk::new(NewPublicKeyJwk {
        kty: KeyType::OKP,
        crv: CurveType::Ed25519,
        x: COORD_32.to_string(),
        y: None,
        extensions,
    })
    .unwrap();
    let value = serde_json::to_value(&jwk).unwrap();
    assert_eq!(
        value,
        json!({ "kty": "OKP", "crv": "Ed25519", "x": COORD_32, "kid": "key-1" })
    );
    let back: PublicKeyJwk = serde_json::from_value(value).unwrap();
    assert_eq!(back, jwk);
}

// ---- VerificationMethod / Service accessors + serde ------------------------

#[test]
fn verification_method_serde_round_trips_with_renamed_fields() {
    let method = vm(&format!("{SUBJECT}#key-1"));
    let value = serde_json::to_value(&method).unwrap();
    assert_eq!(value["type"], json!("JsonWebKey"));
    assert_eq!(value["publicKeyJwk"]["crv"], json!("Ed25519"));
    let back: VerificationMethod = serde_json::from_value(value).unwrap();
    assert_eq!(back, method);
    assert_eq!(back.type_(), VerificationMethodType::JsonWebKey);
    assert_eq!(back.controller().as_str(), SUBJECT);
    assert_eq!(back.public_key_jwk().crv(), CurveType::Ed25519);
}

#[test]
fn service_accessors_expose_type_and_endpoint() {
    let svc = service("#svc-1", ServiceEndpoint::Uri("https://example.com/".into()));
    assert!(matches!(svc.type_(), ServiceType::One(t) if t == "LinkedDomains"));
    assert!(matches!(svc.service_endpoint(), ServiceEndpoint::Uri(u) if u == "https://example.com/"));
}

#[test]
fn service_new_normalizes_endpoint_uris() {
    let svc = service("#svc-1", ServiceEndpoint::Uri("HTTPS://Example.COM:443/".into()));
    assert!(matches!(svc.service_endpoint(), ServiceEndpoint::Uri(u) if u == "https://example.com/"));
}

// ---- normalize_service_endpoint ----------------------------------------------

#[test]
fn normalize_endpoint_walks_objects_recursively() {
    let mut map = serde_json::Map::new();
    map.insert("uri".into(), json!("HTTP://A.example:80/"));
    map.insert("routingKeys".into(), json!(["WSS://R.example:443/"]));
    map.insert("priority".into(), json!(1));
    let normalized = normalize_service_endpoint(ServiceEndpoint::Object(map));
    let ServiceEndpoint::Object(out) = normalized else {
        panic!("object endpoint must stay an object");
    };
    assert_eq!(out["uri"], json!("http://a.example/"));
    assert_eq!(out["routingKeys"], json!(["wss://r.example/"]));
    assert_eq!(out["priority"], json!(1));
}

#[test]
fn normalize_endpoint_walks_array_entries() {
    let mut map = serde_json::Map::new();
    map.insert("uri".into(), json!({ "nested": "HTTP://N.example/" }));
    let normalized = normalize_service_endpoint(ServiceEndpoint::Array(vec![
        ServiceEndpointArrayEntry::Uri("HTTPS://A.example/".into()),
        ServiceEndpointArrayEntry::Object(map),
    ]));
    let ServiceEndpoint::Array(entries) = normalized else {
        panic!("array endpoint must stay an array");
    };
    assert!(matches!(&entries[0], ServiceEndpointArrayEntry::Uri(u) if u == "https://a.example/"));
    let ServiceEndpointArrayEntry::Object(obj) = &entries[1] else {
        panic!("object entry must stay an object");
    };
    assert_eq!(obj["uri"], json!({ "nested": "http://n.example/" }));
}

// ---- DidDocument::validate cross-consistency ---------------------------------

#[test]
fn document_validate_accepts_relative_and_absolute_vm_id_mix() {
    let mut doc = minimal_doc();
    doc.verification_method = Some(vec![vm("#key-1"), vm(&format!("{SUBJECT}#key-2"))]);
    doc.authentication = Some(vec![
        DidKeyId::parse("#key-1").unwrap(),
        DidKeyId::parse(format!("{SUBJECT}#key-2")).unwrap(),
    ]);
    assert!(doc.validate().is_ok());
}

#[test]
fn document_validate_rejects_duplicate_vm_ids_across_spellings() {
    // "#key-1" and "did:...#key-1" canonicalize to the same id.
    let mut doc = minimal_doc();
    doc.verification_method = Some(vec![vm("#key-1"), vm(&format!("{SUBJECT}#key-1"))]);
    let err = doc.validate().unwrap_err();
    assert_eq!(err.issues[0].message, "verificationMethod ids must be unique");
    assert_eq!(err.issues[0].path, vec!["verificationMethod", "1", "id"]);
    assert!(err.summary.contains("at verificationMethod.1.id"));
}

#[test]
fn document_validate_rejects_duplicate_relation_entries() {
    let mut doc = minimal_doc();
    doc.verification_method = Some(vec![vm("#key-1")]);
    doc.authentication = Some(vec![
        DidKeyId::parse("#key-1").unwrap(),
        DidKeyId::parse(format!("{SUBJECT}#key-1")).unwrap(),
    ]);
    let err = doc.validate().unwrap_err();
    assert_eq!(
        err.issues[0].message,
        "authentication must not contain duplicate entries"
    );
    assert_eq!(err.issues[0].path, vec!["authentication", "1"]);
}

#[test]
fn document_validate_rejects_dangling_relation_references() {
    let mut doc = minimal_doc();
    doc.verification_method = Some(vec![vm("#key-1")]);
    doc.key_agreement = Some(vec![DidKeyId::parse("#missing").unwrap()]);
    doc.capability_invocation = Some(vec![DidKeyId::parse("#also-missing").unwrap()]);
    doc.capability_delegation = Some(vec![DidKeyId::parse("#gone").unwrap()]);
    doc.assertion_method = Some(vec![DidKeyId::parse("#nowhere").unwrap()]);
    let err = doc.validate().unwrap_err();
    let messages: Vec<&str> = err.issues.iter().map(|i| i.message.as_str()).collect();
    for relation in [
        "assertionMethod",
        "keyAgreement",
        "capabilityInvocation",
        "capabilityDelegation",
    ] {
        assert!(
            messages
                .iter()
                .any(|m| *m == format!("{relation} references a verificationMethod id that does not exist")),
            "missing dangling-reference issue for {relation}: {messages:?}"
        );
    }
}

#[test]
fn document_validate_rejects_duplicate_service_ids() {
    let mut doc = minimal_doc();
    doc.service = Some(vec![
        service("#svc-1", ServiceEndpoint::Uri("https://a.example/".into())),
        service("#svc-1", ServiceEndpoint::Uri("https://b.example/".into())),
    ]);
    let err = doc.validate().unwrap_err();
    assert_eq!(err.issues[0].message, "service ids must be unique");
    assert_eq!(err.issues[0].path, vec!["service", "1", "id"]);
}

#[test]
fn document_validate_rejects_duplicate_service_endpoints_after_normalization() {
    // The two spellings normalize to the same URI, so the array holds a
    // duplicate endpoint.
    let mut doc = minimal_doc();
    doc.service = Some(vec![service(
        "#svc-1",
        ServiceEndpoint::Array(vec![
            ServiceEndpointArrayEntry::Uri("https://a.example/".into()),
            ServiceEndpointArrayEntry::Uri("HTTPS://A.example:443/".into()),
        ]),
    )]);
    let err = doc.validate().unwrap_err();
    assert_eq!(err.issues[0].message, "serviceEndpoint values must be unique");
    assert_eq!(err.issues[0].path, vec!["service", "0", "serviceEndpoint", "1"]);
}

#[test]
fn with_normalized_service_endpoints_rewrites_in_place() {
    let mut doc = minimal_doc();
    doc.service = Some(vec![
        Service::new(NewService {
            id: "#svc-1".into(),
            type_: ServiceType::One("LinkedDomains".into()),
            service_endpoint: ServiceEndpoint::Uri("https://example.com/".into()),
        })
        .unwrap(),
    ]);
    // Round-trip through JSON to plant an unnormalized endpoint (Service::new
    // would normalize it eagerly).
    let mut value = serde_json::to_value(&doc).unwrap();
    value["service"][0]["serviceEndpoint"] = json!("HTTPS://Example.COM:443/x");
    let doc: DidDocument = serde_json::from_value(value).unwrap();
    let normalized = doc.with_normalized_service_endpoints();
    let services = normalized.service.as_ref().unwrap();
    assert!(matches!(services[0].service_endpoint(), ServiceEndpoint::Uri(u) if u == "https://example.com/x"));
}

// ---- parse_did_document / parse_verification_method / parse_service ----------

#[test]
fn parse_did_document_accepts_full_document_and_keeps_extras() {
    let doc = parse_did_document(json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": SUBJECT,
        "alsoKnownAs": ["https://alias.example/"],
        "controller": SUBJECT,
        "verificationMethod": [{
            "id": format!("{SUBJECT}#key-1"),
            "type": "JsonWebKey",
            "controller": SUBJECT,
            "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": COORD_32 }
        }],
        "authentication": ["#key-1"],
        "service": [{
            "id": "#svc-1",
            "type": "LinkedDomains",
            "serviceEndpoint": "HTTPS://Example.COM:443/"
        }],
        "customExtension": { "hello": "world" }
    }))
    .expect("valid document");
    assert!(matches!(&doc.context, DocumentContext::Many(c) if c.len() == 1));
    assert!(matches!(&doc.controller, Some(Controller::One(c)) if c.as_str() == SUBJECT));
    assert_eq!(doc.extra.get("customExtension"), Some(&json!({ "hello": "world" })));
    // Endpoints come back normalized.
    let services = doc.service.as_ref().unwrap();
    assert!(matches!(services[0].service_endpoint(), ServiceEndpoint::Uri(u) if u == "https://example.com/"));
}

#[test]
fn parse_did_document_rejects_wrong_json_shape() {
    let err = parse_did_document(json!({ "id": 42 })).unwrap_err();
    assert!(err.summary.contains("DID Document JSON shape is invalid"));
}

#[test]
fn parse_did_document_rejects_cross_consistency_failures() {
    let err = parse_did_document(json!({
        "@context": "https://www.w3.org/ns/did/v1",
        "id": SUBJECT,
        "authentication": ["#phantom"]
    }))
    .unwrap_err();
    assert!(
        err.summary
            .contains("authentication references a verificationMethod id that does not exist")
    );
}

#[test]
fn parse_verification_method_accepts_valid_json() {
    let method = parse_verification_method(json!({
        "id": format!("{SUBJECT}#key-1"),
        "type": "JsonWebKey",
        "controller": SUBJECT,
        "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": COORD_32 }
    }))
    .expect("valid VM JSON");
    assert_eq!(method.id().as_str(), format!("{SUBJECT}#key-1"));
}

#[test]
fn parse_verification_method_rejects_wrong_shape() {
    let err = parse_verification_method(json!({ "id": "#key-1" })).unwrap_err();
    assert!(err.summary.contains("VerificationMethod JSON shape is invalid"));
}

#[test]
fn parse_verification_method_rejects_semantic_failures() {
    // Shape is fine (transparent newtypes deserialize unchecked) but the
    // controller is not a DID, so validate() must reject.
    let err = parse_verification_method(json!({
        "id": format!("{SUBJECT}#key-1"),
        "type": "JsonWebKey",
        "controller": "not-a-did",
        "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": COORD_32 }
    }))
    .unwrap_err();
    assert!(err.summary.contains("Invalid DID format"));
}

#[test]
fn parse_service_accepts_and_normalizes() {
    let svc = parse_service(json!({
        "id": "#svc-1",
        "type": ["LinkedDomains", "CredentialRegistry"],
        "serviceEndpoint": ["HTTPS://A.example:443/", { "uri": "WS://B.example:80/" }]
    }))
    .expect("valid service JSON");
    let ServiceEndpoint::Array(entries) = svc.service_endpoint() else {
        panic!("expected array endpoint");
    };
    assert!(matches!(&entries[0], ServiceEndpointArrayEntry::Uri(u) if u == "https://a.example/"));
}

#[test]
fn parse_service_rejects_wrong_shape() {
    let err = parse_service(json!({ "id": "#svc-1" })).unwrap_err();
    assert!(err.summary.contains("Service JSON shape is invalid"));
}

#[test]
fn parse_service_rejects_semantic_failures() {
    let err = parse_service(json!({
        "id": "#svc-1",
        "type": "",
        "serviceEndpoint": "https://example.com/"
    }))
    .unwrap_err();
    assert!(err.summary.contains("service type must not be empty"));
}

// ---- DidResolutionErrorCode ----------------------------------------------------

#[test]
fn resolution_error_code_accepts_keyword_grammar() {
    assert!(DidResolutionErrorCode("notFound".into()).validate().is_ok());
    assert!(DidResolutionErrorCode("customError123".into()).validate().is_ok());
}

#[test]
fn resolution_error_code_rejects_non_keywords() {
    for bad in ["", "1abc", "not-found", "has space"] {
        let err = DidResolutionErrorCode(bad.into()).validate().unwrap_err();
        assert!(
            err.summary.contains("DID resolution error must match"),
            "expected keyword-grammar error for {bad:?}"
        );
    }
}

// ---- Metadata + resolution envelopes ---------------------------------------------

#[test]
fn did_document_metadata_default_serializes_to_empty_object() {
    let metadata = DidDocumentMetadata::default();
    assert_eq!(serde_json::to_value(&metadata).unwrap(), json!({}));
}

#[test]
fn did_document_metadata_round_trips_all_fields_and_extras() {
    let value = json!({
        "created": "2026-01-01T00:00:00Z",
        "updated": "2026-02-01T00:00:00Z",
        "deactivated": false,
        "versionId": "7",
        "nextUpdate": "2026-03-01T00:00:00Z",
        "nextVersionId": "8",
        "equivalentId": ["did:midnight:testnet:equiv"],
        "canonicalId": SUBJECT,
        "customHint": "extension"
    });
    let metadata: DidDocumentMetadata = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(metadata.version_id.as_deref(), Some("7"));
    assert_eq!(metadata.canonical_id.as_deref(), Some(SUBJECT));
    assert_eq!(metadata.extra.get("customHint"), Some(&json!("extension")));
    assert_eq!(serde_json::to_value(&metadata).unwrap(), value);
}

#[test]
fn did_resolution_result_round_trips() {
    // alsoKnownAs/service serialize as explicit nulls when absent
    // (TS parity, issue #15) — deserialization accepts both null and
    // missing, so the round-trip is asymmetric by design.
    let value = json!({
        "@context": "https://w3id.org/did-resolution/v1",
        "didDocument": {
            "@context": "https://www.w3.org/ns/did/v1",
            "alsoKnownAs": null,
            "id": SUBJECT,
            "service": null
        },
        "didDocumentMetadata": { "deactivated": true },
        "didResolutionMetadata": {
            "contentType": "application/did+ld+json",
            "error": "notFound"
        }
    });
    let result: DidResolutionResult = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(result.did_document.as_ref().unwrap().id.as_str(), SUBJECT);
    assert_eq!(result.did_document_metadata.deactivated, Some(true));
    assert_eq!(
        result.did_resolution_metadata.content_type,
        Some(KnownDidMediaType::DidLdJson)
    );
    assert_eq!(
        result.did_resolution_metadata.error,
        Some(DidResolutionErrorCode("notFound".into()))
    );
    assert_eq!(serde_json::to_value(&result).unwrap(), value);
}

#[test]
fn did_resolution_metadata_default_is_empty() {
    let metadata = DidResolutionMetadata::default();
    assert!(metadata.content_type.is_none());
    assert!(metadata.error.is_none());
    assert_eq!(serde_json::to_value(&metadata).unwrap(), json!({}));
}

// ---- create_did_document -----------------------------------------------------------

#[test]
fn create_did_document_defaults_context() {
    let doc = create_did_document(CreateDidDocumentParams {
        id: SUBJECT.to_string(),
        ..Default::default()
    })
    .expect("minimal params valid");
    assert!(matches!(&doc.context, DocumentContext::One(c) if c == "https://www.w3.org/ns/did/v1"));
}

#[test]
fn create_did_document_threads_relations_and_services() {
    let key = vm("#key-1");
    let doc = create_did_document(CreateDidDocumentParams {
        id: SUBJECT.to_string(),
        context: Some(DocumentContext::Many(vec![
            "https://www.w3.org/ns/did/v1".into(),
            "https://w3id.org/security/jwk/v1".into(),
        ])),
        controller: Some(Controller::Many(vec![DidString::parse(SUBJECT).unwrap()])),
        verification_method: Some(vec![key]),
        authentication: Some(vec![DidKeyId::parse("#key-1").unwrap()]),
        service: Some(vec![service(
            "#svc-1",
            ServiceEndpoint::Uri("https://a.example/".into()),
        )]),
        ..Default::default()
    })
    .expect("full params valid");
    assert!(matches!(&doc.context, DocumentContext::Many(c) if c.len() == 2));
    assert!(matches!(&doc.controller, Some(Controller::Many(c)) if c.len() == 1));
}

#[test]
fn create_did_document_rejects_bad_subject() {
    let err = create_did_document(CreateDidDocumentParams {
        id: "not-a-did".to_string(),
        ..Default::default()
    })
    .unwrap_err();
    assert!(err.summary.contains("Invalid DID format"));
}

#[test]
fn create_did_document_rejects_dangling_relations() {
    let err = create_did_document(CreateDidDocumentParams {
        id: SUBJECT.to_string(),
        authentication: Some(vec![DidKeyId::parse("#phantom").unwrap()]),
        ..Default::default()
    })
    .unwrap_err();
    assert!(
        err.summary
            .contains("authentication references a verificationMethod id that does not exist")
    );
}

// ---- Builder edges not covered in did_document_builder.rs ---------------------------

#[test]
fn builder_context_also_known_as_and_controller_are_threaded() {
    let doc = DidDocumentBuilder::new(SUBJECT)
        .context(DocumentContext::Many(vec!["https://www.w3.org/ns/did/v1".into()]))
        .also_known_as(vec!["https://alias.example/".into()])
        .controller(Controller::One(DidString::parse(SUBJECT).unwrap()))
        .build()
        .expect("valid");
    assert!(matches!(&doc.context, DocumentContext::Many(_)));
    assert_eq!(doc.also_known_as.as_ref().unwrap()[0], "https://alias.example/");
    assert!(matches!(&doc.controller, Some(Controller::One(c)) if c.as_str() == SUBJECT));
}

#[test]
fn builder_rejects_duplicate_service_ids() {
    let s1 = service("#svc-1", ServiceEndpoint::Uri("https://a.example/".into()));
    let s2 = service("#svc-1", ServiceEndpoint::Uri("https://b.example/".into()));
    let err = DidDocumentBuilder::new(SUBJECT)
        .add_service(s1)
        .add_service(s2)
        .build()
        .unwrap_err();
    assert!(format!("{err}").contains("duplicate service id: #svc-1"));
}

#[test]
fn builder_rejects_key_agreement_referencing_unknown_vm() {
    let phantom = DidKeyId::parse(format!("{SUBJECT}#phantom")).unwrap();
    let err = DidDocumentBuilder::new(SUBJECT)
        .key_agreement(vec![phantom])
        .build()
        .unwrap_err();
    assert!(format!("{err}").contains("keyAgreement references unknown verificationMethod id"));
}
