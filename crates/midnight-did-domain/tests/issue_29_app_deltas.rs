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
    assert_eq!(KnownDidResolutionErrorCode::UnsupportedPublicKeyType.http_status(), 501);
}
