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

//! TS↔Rust parity for the generated digital-passport circuits.
//!
//! `tests/fixtures/ts-parity.json` holds golden vectors captured once from
//! the released `@midnight-ntwrk/midnight-vc-passport` package (its TS target
//! of the same Compact sources) by `tests/fixtures/capture-ts-parity.mjs`;
//! its `source` block records the exact package versions. The Rust side
//! rebuilds the same fixture independently through `support` and must
//! reproduce every value: claim commitments (`persistentCommit`), the claim
//! and body roots (`persistentHash`), the proof challenges, and the
//! accept / reject verdicts of the validation circuits.
//!
//! A mismatch means the Rust codegen and the TS target disagree on the same
//! source — the silent-wrong-hash class that drift checks cannot catch.
//! Re-capture only when the pinned package changes.

#![cfg(feature = "digital-passport")]

#[allow(dead_code)] // the civil-date helpers serve the smoke tests only
mod support;

use midnight_compact_runtime::{CompactError, Fr, JubjubPoint, construct_jubjub_point, ec_mul_generator};
use midnight_vc_families::contract::digital_passport::{Proof, Signature, VerificationMethodRef, pure_circuits};
use serde_json::Value;
use sha2::{Digest, Sha256};
use support::create_digital_passport_fixture;

const VECTORS: &str = include_str!("fixtures/ts-parity.json");

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("ts-parity.json parses")
}

fn bytes32(value: &Value) -> [u8; 32] {
    let text = value.as_str().expect("hex string");
    hex::decode(text).expect("hex").try_into().expect("32 bytes")
}

fn field(value: &Value) -> Fr {
    Fr::from_le_bytes(&bytes32(value)).expect("canonical field element")
}

fn point(value: &Value) -> JubjubPoint {
    construct_jubjub_point(field(&value["x"]), field(&value["y"]))
}

fn sha256(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

/// Rebuild one of upstream's `signProof` outputs: every field but the
/// signature comes from the Rust fixture; the Schnorr response `s` (and the
/// points it depends on) come from the vectors, since the circuits verify
/// rather than produce it.
fn proof(signer: VerificationMethodRef, created_at: u64, challenge_hash: [u8; 32], captured: &Value) -> Proof {
    Proof {
        signerVerificationMethodRef: signer,
        createdAt: created_at,
        challengeHash: challenge_hash,
        publicKey: point(&captured["publicKey"]),
        signature: Signature {
            r: point(&captured["signatureR"]),
            s: field(&captured["signatureS"]),
        },
    }
}

fn credential_proof(fixture: &support::DigitalPassportFixture, v: &Value) -> Proof {
    // upstream: signProof({ context: 'issuance', createdAt: 10001n,
    // challengeHash: sha256('challenge:issuance'), nonceScalar: 11n }).
    proof(
        fixture.credential.issuerVerificationMethodRef.clone(),
        10_001,
        sha256("challenge:issuance"),
        &v["credentialProof"],
    )
}

fn presentation_proof(fixture: &support::DigitalPassportFixture, v: &Value) -> Proof {
    // upstream: signProof({ context: 'presentation', createdAt: 10100n,
    // challengeHash: sha256('challenge:verifier'), nonceScalar: 17n }).
    proof(
        fixture.credential.holderBinding.holderVerificationMethodRef.clone(),
        10_100,
        sha256("challenge:verifier"),
        &v["presentationProof"],
    )
}

/// The claim commitments go through `persistentCommit`, whose
/// argument-typing bug (MediaNoxLabs/compact#92) produced wrong hashes
/// with no build or runtime signal.
#[test]
fn claim_commitments_and_root_match_ts() {
    let v = vectors();
    let fixture = create_digital_passport_fixture();
    let c = &fixture.claim_commitments;
    let expected = &v["claimCommitments"];
    assert_eq!(c.firstNameCommitment, bytes32(&expected["firstNameCommitment"]));
    assert_eq!(c.lastNameCommitment, bytes32(&expected["lastNameCommitment"]));
    assert_eq!(c.dateOfBirthCommitment, bytes32(&expected["dateOfBirthCommitment"]));
    assert_eq!(
        c.documentNumberCommitment,
        bytes32(&expected["documentNumberCommitment"])
    );
    assert_eq!(c.issuingStateCommitment, bytes32(&expected["issuingStateCommitment"]));
    assert_eq!(fixture.credential.claimRoot, bytes32(&v["claimRoot"]));
}

#[test]
fn body_roots_match_ts() {
    let v = vectors();
    let fixture = create_digital_passport_fixture();
    assert_eq!(
        pure_circuits::digital_passport_credential_body_root(fixture.credential.clone()).expect("credential body root"),
        bytes32(&v["credentialBodyRoot"])
    );
    assert_eq!(
        pure_circuits::digital_passport_presentation_body_root(fixture.presentation.clone())
            .expect("presentation body root"),
        bytes32(&v["presentationBodyRoot"])
    );
}

/// upstream's `createSigner('issuer', 123456789n)` / `('holder', 987654321n)`
/// keys and the `nonceScalar` announcements, derived on the Rust side.
#[test]
fn signer_points_match_ts() {
    let v = vectors();
    assert_eq!(
        point(&v["credentialProof"]["publicKey"]),
        ec_mul_generator(Fr::from(123_456_789u64))
    );
    assert_eq!(
        point(&v["credentialProof"]["signatureR"]),
        ec_mul_generator(Fr::from(11u64))
    );
    assert_eq!(
        point(&v["presentationProof"]["publicKey"]),
        ec_mul_generator(Fr::from(987_654_321u64))
    );
    assert_eq!(
        point(&v["presentationProof"]["signatureR"]),
        ec_mul_generator(Fr::from(17u64))
    );
}

/// upstream's `signProof` hashes the proof with `s = 0`.
#[test]
fn proof_challenges_match_ts() {
    let v = vectors();
    let fixture = create_digital_passport_fixture();

    let mut issuance = credential_proof(&fixture, &v);
    issuance.signature.s = Fr::from(0u64);
    let credential_body_root =
        pure_circuits::digital_passport_credential_body_root(fixture.credential.clone()).expect("credential body root");
    assert_eq!(
        pure_circuits::issuance_proof_challenge(credential_body_root, issuance).expect("issuance challenge"),
        field(&v["issuanceChallenge"])
    );

    let mut presentation = presentation_proof(&fixture, &v);
    presentation.signature.s = Fr::from(0u64);
    let presentation_body_root = pure_circuits::digital_passport_presentation_body_root(fixture.presentation.clone())
        .expect("presentation body root");
    assert_eq!(
        pure_circuits::presentation_proof_challenge(presentation_body_root, presentation)
            .expect("presentation challenge"),
        field(&v["presentationChallenge"])
    );
}

#[test]
fn validation_verdicts_match_ts() {
    let v = vectors();
    let fixture = create_digital_passport_fixture();
    let credential_proof = credential_proof(&fixture, &v);

    pure_circuits::assert_valid_digital_passport_credential(fixture.credential.clone(), credential_proof.clone())
        .expect("TS accepts the fixture credential");
    pure_circuits::assert_valid_digital_passport_presentation(
        fixture.credential.clone(),
        credential_proof.clone(),
        fixture.presentation.clone(),
        presentation_proof(&fixture, &v),
    )
    .expect("TS accepts the fixture presentation");

    let mut tampered = fixture.credential.clone();
    tampered.claimRoot[0] ^= 0x01;
    let expected = v["tamperedClaimRootRejection"].as_str().expect("rejection message");
    let expected = expected.strip_prefix("failed assert: ").unwrap_or(expected);
    match pure_circuits::assert_valid_digital_passport_credential(tampered, credential_proof) {
        Err(CompactError::AssertionFailed(message)) => assert_eq!(message, expected),
        other => panic!("expected the TS rejection {expected:?}, got {other:?}"),
    }
}
