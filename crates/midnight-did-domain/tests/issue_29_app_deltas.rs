// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use midnight_did_domain::{
    CurveType, DidDocument, DidKeyId, DidResolutionErrorCode, KnownDidResolutionErrorCode, MAX_DID_DOCUMENT_ENTRIES,
    MAX_DID_STRING_BYTES, NewPublicKeyJwk, NewService, NewVerificationMethod, PublicKeyJwk, Service, ServiceEndpoint,
    ServiceType, VerificationMethod, VerificationMethodType, parse_did_document,
};
use serde_json::json;

fn jwk() -> PublicKeyJwk {
    PublicKeyJwk::new(NewPublicKeyJwk {
        kty: midnight_did_domain::KeyType::OKP,
        crv: CurveType::Ed25519,
        x: "A".repeat(43),
        y: None,
        extensions: BTreeMap::new(),
    })
    .unwrap()
}

#[test]
fn verification_method_unknown_fields_round_trip_losslessly() {
    let value = json!({
        "id": "did:example:alice#key-1",
        "type": "JsonWebKey",
        "controller": "did:example:alice",
        "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" },
        "blockchainAccountId": "eip155:1:0xabc",
        "nested": { "keep": [true, 7, "value"] }
    });

    let method: VerificationMethod = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(method.extensions()["blockchainAccountId"], "eip155:1:0xabc");
    assert_eq!(serde_json::to_value(&method).unwrap(), value);
}

#[test]
fn did_document_and_verification_method_extensions_survive_parse_serialize() {
    let value = json!({
        "@context": "https://www.w3.org/ns/did/v1",
        "id": "did:example:alice",
        "alsoKnownAs": ["did:example:alias"],
        "verificationMethod": [{
            "id": "did:example:alice#key-1",
            "type": "JsonWebKey",
            "controller": "did:example:alice",
            "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" },
            "portalExtension": { "roundTrip": true }
        }],
        "authentication": ["#key-1"],
        "service": null,
        "portalDocumentExtension": [1, 2, 3]
    });

    let doc = parse_did_document(value.clone()).unwrap();
    assert_eq!(doc.extra["portalDocumentExtension"], json!([1, 2, 3]));
    assert_eq!(serde_json::to_value(&doc).unwrap(), value);
}

#[test]
fn direct_deserialization_runs_validation_gates() {
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice\n",
            "verificationMethod": []
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<VerificationMethod>(json!({
            "id": "did:example:alice",
            "type": "JsonWebKey",
            "controller": "did:example:alice",
            "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" }
        }))
        .is_err()
    );
}

#[test]
fn bounded_inputs_and_entry_counts_are_enforced_at_edges() {
    assert!(midnight_did_domain::parse_did(&format!("did:example:{}", "a".repeat(MAX_DID_STRING_BYTES))).is_err());

    let too_many_methods = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|i| {
            json!({
                "id": format!("did:example:alice#key-{i}"),
                "type": "JsonWebKey",
                "controller": "did:example:alice",
                "publicKeyJwk": { "kty": "OKP", "crv": "Ed25519", "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA" }
            })
        })
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "verificationMethod": too_many_methods
        }))
        .is_err()
    );
}

#[test]
fn oxid_style_relationship_and_service_shapes_map_cleanly() {
    let method = VerificationMethod::new_with_extensions(
        NewVerificationMethod {
            id: "#key-1".to_owned(),
            type_: VerificationMethodType::JsonWebKey,
            controller: "did:example:alice".to_owned(),
            public_key_jwk: jwk(),
        },
        BTreeMap::from([("x-oxid".to_owned(), json!(true))]),
    )
    .unwrap();
    assert_eq!(method.extensions()["x-oxid"], true);

    let service = Service::new(NewService {
        id: "#svc".to_owned(),
        type_: ServiceType::Many(vec!["LinkedDomains".to_owned(), "DIDCommMessaging".to_owned()]),
        service_endpoint: ServiceEndpoint::Uri("HTTPS://EXAMPLE.COM".to_owned()),
    })
    .unwrap();
    assert!(matches!(service.type_(), ServiceType::Many(values) if values.len() == 2));

    let key_id = DidKeyId::parse("#key-1").unwrap();
    let doc = midnight_did_domain::DidDocumentBuilder::new("did:example:alice")
        .add_verification_method(method)
        .authentication(vec![key_id])
        .add_service(service)
        .build()
        .unwrap();
    assert!(doc.validate().is_ok());
}

#[test]
fn did_resolution_error_http_status_classifier_matches_identus_mapping() {
    let cases = [
        ("invalidDid", 400),
        ("invalidDidUrl", 400),
        ("invalidOptions", 400),
        ("notFound", 404),
        ("deactivated", 410),
        ("representationNotSupported", 406),
        ("methodNotSupported", 501),
        ("unsupportedPublicKeyType", 501),
        ("internalError", 500),
        ("extensionProblem", 500),
    ];

    for (code, status) in cases {
        assert_eq!(DidResolutionErrorCode(code.to_owned()).http_status(), status, "{code}");
    }

    for (known, status) in [
        (KnownDidResolutionErrorCode::InvalidDid, 400),
        (KnownDidResolutionErrorCode::NotFound, 404),
        (KnownDidResolutionErrorCode::RepresentationNotSupported, 406),
        (KnownDidResolutionErrorCode::MethodNotSupported, 501),
        (KnownDidResolutionErrorCode::UnsupportedPublicKeyType, 501),
        (KnownDidResolutionErrorCode::InternalError, 500),
    ] {
        assert_eq!(known.http_status(), status, "{known:?}");
    }

    // These standardized keywords are intentionally generic-only so the
    // exhaustive known enum remains source-compatible for downstream matches.
    for generic_only in ["invalidDidUrl", "invalidOptions", "deactivated"] {
        assert!(serde_json::from_value::<KnownDidResolutionErrorCode>(json!(generic_only)).is_err());
    }
}

#[test]
fn document_public_text_limits_are_exact_and_control_safe() {
    let exact_did = format!(
        "did:example:{}",
        "a".repeat(MAX_DID_STRING_BYTES - "did:example:".len())
    );
    assert!(midnight_did_domain::parse_did(&exact_did).is_ok());
    assert!(midnight_did_domain::parse_did(&format!("{exact_did}x")).is_err());

    let exact_context = format!("x:{}", "a".repeat(midnight_did_domain::MAX_DID_DOCUMENT_TEXT_BYTES - 2));
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": exact_context,
            "id": "did:example:alice"
        }))
        .is_ok()
    );

    for bad_context in [
        "",
        " https://www.w3.org/ns/did/v1",
        "https://www.w3.org/ns/did/v1\n",
        "not-a-uri",
    ] {
        assert!(
            serde_json::from_value::<DidDocument>(json!({
                "@context": bad_context,
                "id": "did:example:alice"
            }))
            .is_err(),
            "context={bad_context:?}"
        );
    }

    for bad_alias in ["", " did:example:alias", "did:example:alias\n", "not-a-uri"] {
        assert!(
            serde_json::from_value::<DidDocument>(json!({
                "@context": "https://www.w3.org/ns/did/v1",
                "id": "did:example:alice",
                "alsoKnownAs": [bad_alias]
            }))
            .is_err(),
            "alias={bad_alias:?}"
        );
    }
}

#[test]
fn document_list_cardinality_is_enforced_for_context_aliases_and_controller() {
    let max_contexts = (0..MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| format!("https://example.com/context/{index}"))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": max_contexts,
            "id": "did:example:alice"
        }))
        .is_ok()
    );

    let too_many_contexts = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| format!("https://example.com/context/{index}"))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": too_many_contexts,
            "id": "did:example:alice"
        }))
        .is_err()
    );

    let too_many_aliases = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| format!("did:example:alias{index}"))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "alsoKnownAs": too_many_aliases
        }))
        .is_err()
    );

    let too_many_controllers = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| format!("did:example:controller{index}"))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "controller": too_many_controllers
        }))
        .is_err()
    );

    for field in ["@context", "alsoKnownAs", "controller"] {
        let mut doc = json!({ "@context": "https://www.w3.org/ns/did/v1", "id": "did:example:alice" });
        doc[field] = json!([]);
        assert!(serde_json::from_value::<DidDocument>(doc).is_err(), "field={field}");
    }
}

#[test]
fn service_endpoint_uri_boundaries_and_invalid_text_are_rejected() {
    let exact_uri = format!("x:{}", "a".repeat(midnight_did_domain::MAX_DID_DOCUMENT_TEXT_BYTES - 2));
    assert!(
        Service::new(NewService {
            id: "#svc".to_owned(),
            type_: ServiceType::One("LinkedDomains".to_owned()),
            service_endpoint: ServiceEndpoint::Uri(exact_uri),
        })
        .is_ok()
    );

    for bad in ["", " https://example.com", "https://example.com\n", "not-a-uri"] {
        assert!(
            Service::new(NewService {
                id: "#svc".to_owned(),
                type_: ServiceType::One("LinkedDomains".to_owned()),
                service_endpoint: ServiceEndpoint::Uri(bad.to_owned()),
            })
            .is_err(),
            "endpoint={bad:?}"
        );
    }

    let overlong = format!("x:{}", "a".repeat(midnight_did_domain::MAX_DID_DOCUMENT_TEXT_BYTES - 1));
    assert!(
        Service::new(NewService {
            id: "#svc".to_owned(),
            type_: ServiceType::One("LinkedDomains".to_owned()),
            service_endpoint: ServiceEndpoint::Uri(overlong),
        })
        .is_err()
    );
}

#[test]
fn service_endpoint_array_and_nested_json_are_bounded_and_control_safe() {
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": []
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": ["https://example.com/one", "not-a-uri"]
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": {
                "uri": "https://example.com/endpoint",
                "routingKeys": ["did:example:alice#key-1"],
                "nested": { "label": "ok" }
            }
        }))
        .is_ok()
    );

    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": { "label": "bad\nvalue" }
        }))
        .is_err()
    );

    let oversized = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| (format!("k{index}"), json!(index)))
        .collect::<serde_json::Map<_, _>>();
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": oversized
        }))
        .is_err()
    );
}

#[test]
fn public_key_jwk_extensions_are_bounded_at_construction_and_deserialization() {
    let overlong = "a".repeat(midnight_did_domain::MAX_DID_DOCUMENT_TEXT_BYTES + 1);
    assert!(
        PublicKeyJwk::new(NewPublicKeyJwk {
            kty: midnight_did_domain::KeyType::OKP,
            crv: CurveType::Ed25519,
            x: "A".repeat(43),
            y: None,
            extensions: BTreeMap::from([("extension".to_owned(), json!(overlong))]),
        })
        .is_err()
    );

    let too_many = (0..=MAX_DID_DOCUMENT_ENTRIES)
        .map(|index| (format!("k{index}"), json!(index)))
        .collect::<BTreeMap<_, _>>();
    assert!(
        PublicKeyJwk::new(NewPublicKeyJwk {
            kty: midnight_did_domain::KeyType::OKP,
            crv: CurveType::Ed25519,
            x: "A".repeat(43),
            y: None,
            extensions: too_many,
        })
        .is_err()
    );

    let nested =
        (0..=midnight_did_domain::MAX_DID_DOCUMENT_EXTENSION_DEPTH).fold(json!("leaf"), |value, _| json!([value]));
    assert!(
        serde_json::from_value::<VerificationMethod>(json!({
            "id": "did:example:alice#key-1",
            "type": "JsonWebKey",
            "controller": "did:example:alice",
            "publicKeyJwk": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "extension": nested
            }
        }))
        .is_err()
    );
}

#[test]
fn service_endpoint_node_budget_is_shared_across_array_entries() {
    let valid_entries = (0..100)
        .map(|index| json!({ "items": [index, index, index, index, index, index, index, index] }))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": valid_entries
        }))
        .is_ok()
    );

    let too_many_nodes = (0..=102)
        .map(|index| json!({ "items": [index, index, index, index, index, index, index, index] }))
        .collect::<Vec<_>>();
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": too_many_nodes
        }))
        .is_err()
    );
}

#[test]
fn absolute_uri_fields_reject_malformed_remainders() {
    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://exa mple.com/context",
            "id": "did:example:alice"
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "alsoKnownAs": ["https://exa mple.com/alice"]
        }))
        .is_err()
    );

    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": "https://exa mple.com/endpoint"
        }))
        .is_err()
    );
}

#[test]
fn service_endpoint_uri_array_entries_count_toward_shared_node_budget() {
    let object_entries = (0..102)
        .map(|index| json!({ "items": [index, index, index, index, index, index, index, index] }))
        .collect::<Vec<_>>();

    let mut just_within_budget = object_entries.clone();
    just_within_budget.extend([
        json!("https://example.com/one"),
        json!("https://example.com/two"),
        json!("https://example.com/three"),
    ]);
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": just_within_budget
        }))
        .is_ok()
    );

    let mut over_budget = object_entries;
    over_budget.extend([
        json!("https://example.com/one"),
        json!("https://example.com/two"),
        json!("https://example.com/three"),
        json!("https://example.com/four"),
    ]);
    assert!(
        serde_json::from_value::<Service>(json!({
            "id": "#svc",
            "type": "DIDCommMessaging",
            "serviceEndpoint": over_budget
        }))
        .is_err()
    );
}

fn extension_node_payload() -> serde_json::Value {
    json!(
        (0..MAX_DID_DOCUMENT_ENTRIES)
            .map(|index| json!([format!("leaf-{index}")]))
            .collect::<Vec<_>>()
    )
}

fn extension_byte_payload() -> serde_json::Value {
    let chunk = "x".repeat(midnight_did_domain::MAX_DID_DOCUMENT_TEXT_BYTES);
    json!((0..33).map(|_| json!(chunk)).collect::<Vec<_>>())
}

#[test]
fn document_extension_nodes_are_bounded_across_all_extension_containers() {
    let payload = extension_node_payload();
    assert!(
        serde_json::from_value::<VerificationMethod>(json!({
            "id": "did:example:alice#key-1",
            "type": "JsonWebKey",
            "controller": "did:example:alice",
            "publicKeyJwk": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "jwkExt": payload.clone()
            },
            "vmExt": payload.clone()
        }))
        .is_ok()
    );

    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "docExt": payload.clone(),
            "verificationMethod": [{
                "id": "did:example:alice#key-1",
                "type": "JsonWebKey",
                "controller": "did:example:alice",
                "publicKeyJwk": {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "jwkExt": payload.clone()
                },
                "vmExt": payload.clone()
            }, {
                "id": "did:example:alice#key-2",
                "type": "JsonWebKey",
                "controller": "did:example:alice",
                "publicKeyJwk": {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                },
                "vmExt": payload
            }]
        }))
        .is_err()
    );
}

#[test]
fn document_extension_bytes_are_bounded_across_all_extension_containers() {
    let payload = extension_byte_payload();
    assert!(
        serde_json::from_value::<VerificationMethod>(json!({
            "id": "did:example:alice#key-1",
            "type": "JsonWebKey",
            "controller": "did:example:alice",
            "publicKeyJwk": {
                "kty": "OKP",
                "crv": "Ed25519",
                "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                "jwkExt": payload.clone()
            },
            "vmExt": payload.clone()
        }))
        .is_ok()
    );

    assert!(
        serde_json::from_value::<DidDocument>(json!({
            "@context": "https://www.w3.org/ns/did/v1",
            "id": "did:example:alice",
            "docExt": payload.clone(),
            "verificationMethod": [{
                "id": "did:example:alice#key-1",
                "type": "JsonWebKey",
                "controller": "did:example:alice",
                "publicKeyJwk": {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                    "jwkExt": payload.clone()
                },
                "vmExt": payload.clone()
            }, {
                "id": "did:example:alice#key-2",
                "type": "JsonWebKey",
                "controller": "did:example:alice",
                "publicKeyJwk": {
                    "kty": "OKP",
                    "crv": "Ed25519",
                    "x": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
                },
                "vmExt": payload
            }]
        }))
        .is_err()
    );
}
