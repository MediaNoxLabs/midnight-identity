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

use base64::Engine as _;
use base64::engine::general_purpose;
use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine};
use midnight_vc_proof::{
    IssuanceProof, IssuerKeyMaterial, VerificationMethodRef, VerificationOutcome, VerificationStageName,
    VerificationStageStatus, decode_detached_proof, digital_passport_body_root, encode_detached_proof,
    issuance_challenge, sign_with_rng, verification_method_ref, verify_body_root, verify_digital_passport,
};
use rand::{CryptoRng, RngCore};

// Apache-2.0 fixture copied from MediaNoxLabs/oxid@c9b9bc677836f19a5cd28306d8584a7d8623fd57
// fixtures/credentials/standalone-digital-passport-compact-body.b64.
const OXID_STANDALONE_BODY_B64: &str = "TUNWMQAAABIAAAABAQAAABxtaWRuaWdodDp2YzpkaWdpdGFsLXBhc3Nwb3J0AAAAE2RpZ2l0YWwtcGFzc3BvcnQ6djEAAAABAQAAAAAAAAAgpMlIOgx83YCAVqkzNKuXIHs4tDY9HaXL+3itJWzWifAAAAANI2lzc3Vlci1rZXktMQAAACD3qfy3lbl4uoSoNgGj8MYhl+Y6CxAmDvKhM4arkgGbrAAAAA0jaG9sZGVyLWtleS0xAAAAAhAnAAAAAQEAAAACIE4AAAAgHiI64YIgigX47OPDxwWC0YP8COtsTh4Qq6rSUafGwqAAAAAgCfKXaredeIK0eW7BcqqLck58Qy0+/KcM3RdqUrFuStMAAAAgLfuoiC70uZgmRiXKOLTsy/pYh65XLY+ujsgMk59VHQcAAAAgbLBDeUIW8cbiSE1kRM2+cuouDS0tgDh/BE/t+tk3k5kAAAAgeUUG3M0PUIefWbg5og4R/mYrolm0nxf4UvsXCnG2wlgAAAAgoL5QxKvdQefq7/dPXI0oVrDs7dAXhpE/GfySNFqVX8M=";
// Apache-2.0 fixture copied from MediaNoxLabs/oxid@c9b9bc677836f19a5cd28306d8584a7d8623fd57
// fixtures/credentials/standalone-digital-passport-compact-proof.b64.
const OXID_STANDALONE_PROOF_B64: &str = "TUNWMQAAAAkAAAAgpMlIOgx83YCAVqkzNKuXIHs4tDY9HaXL+3itJWzWifAAAAANI2lzc3Vlci1rZXktMQAAAAIRJwAAACARnDu13/s+TOdUedef2+xNjIowS7BJGnysCu2Ux0n4VAAAAB+vdLcq4BXZjbC+JqDGpOwo25QWaoeVWN9bBC9lpX+lAAAAIG/BnsK7zMOYZXceHQR2UgKgYWG/6e21XXFagthVNPJjAAAAILQr7FnBB09v+rgAjh1AXVzHD492ugj8+DE8tUaPJzQlAAAAILKDVaDWM9CdxJj3Xco4Ue+beju87f0LqdDsDASj01hhAAAAIO+u8acaYsEFijMeu6qgAIPT1L0xlegbPFLshOrbWNwN";
const OXID_BODY_ROOT: [u8; 32] = [
    0xb4, 0x2f, 0x11, 0x15, 0x04, 0x2c, 0xef, 0xec, 0xbd, 0x53, 0x80, 0xa0, 0xa6, 0x30, 0xc0, 0xef, 0x5f, 0x18, 0xbb,
    0x13, 0xe7, 0x61, 0x5c, 0xb1, 0xde, 0x9d, 0x36, 0x25, 0x6f, 0x10, 0x04, 0x32,
];

// Factual fixed challenge vector from input-output-hk/lace-id-portal@f79cc5e070b1634b603baeb73439c259ce23eeb4,
// `crates/did-manager/tests/challenge_parity.rs`: the second fixed vector uses bodyRoot=[1;32],
// didContractAddress=[0x11;32], methodId="#key-assert" right-padded to 32 bytes, createdAt=123,
// challengeHash=[2;32], publicKey=(987654321)*G, and R=(11)*G. The expected challenge bytes below
// were independently derived with this crate's generated `midnight-vc-runtime` challenge delegation; no
// unlicensed lace-id-portal source expression or fixture blob is copied.
const LACE_FIXED_CHALLENGE_LE: [u8; 32] = [
    0xca, 0x14, 0x1f, 0x85, 0x1d, 0x78, 0x22, 0x02, 0xc5, 0xe4, 0x2b, 0x2c, 0x51, 0xd4, 0x20, 0x16, 0xb3, 0x83, 0x44,
    0xe7, 0x0d, 0xc4, 0x61, 0x7e, 0xc1, 0xd7, 0x70, 0x38, 0x49, 0xcb, 0x4a, 0x00,
];

fn fixture(value: &str) -> Vec<u8> {
    general_purpose::STANDARD
        .decode(value)
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(value))
        .expect("base64 fixture")
}

#[test]
fn oxid_fixture_body_root_and_issuance_proof_verify() {
    let body = fixture(OXID_STANDALONE_BODY_B64);
    let proof = fixture(OXID_STANDALONE_PROOF_B64);

    assert_eq!(digital_passport_body_root(&body).expect("body root"), OXID_BODY_ROOT);
    let report = verify_digital_passport(&body, &proof);
    assert_eq!(report.outcome, VerificationOutcome::Valid);
    assert!(
        report
            .stages
            .iter()
            .all(|stage| stage.status == VerificationStageStatus::Passed)
    );
}

#[test]
fn detached_proof_codec_is_byte_canonical_for_oxid_fixture() {
    let proof = fixture(OXID_STANDALONE_PROOF_B64);
    let decoded = decode_detached_proof(&proof).expect("proof");
    assert_eq!(encode_detached_proof(&decoded).expect("encode"), proof);
}

#[test]
fn verification_report_distinguishes_parse_and_signature_failures() {
    let body = fixture(OXID_STANDALONE_BODY_B64);
    let proof = fixture(OXID_STANDALONE_PROOF_B64);
    let body_root = digital_passport_body_root(&body).expect("body root");

    let malformed = verify_body_root(body_root, b"not-mcv1");
    assert_eq!(malformed.outcome, VerificationOutcome::Invalid);
    assert_eq!(malformed.stages[0].name, VerificationStageName::Parse);
    assert_eq!(malformed.stages[0].status, VerificationStageStatus::Failed);

    let mut tampered = proof.clone();
    *tampered.last_mut().expect("proof byte") ^= 1;
    let invalid = verify_body_root(body_root, &tampered);
    assert_eq!(invalid.outcome, VerificationOutcome::Invalid);
    assert_eq!(
        invalid
            .stages
            .iter()
            .find(|stage| stage.name == VerificationStageName::Signature)
            .expect("signature stage")
            .status,
        VerificationStageStatus::Failed
    );
}

#[test]
fn lace_portal_fixed_challenge_vector_matches_generated_runtime() {
    let mut method_id = [0u8; 32];
    method_id[..b"#key-assert".len()].copy_from_slice(b"#key-assert");
    let proof = IssuanceProof {
        signer: VerificationMethodRef {
            did_contract_address: [0x11; 32],
            method_id,
        },
        created_at: 123,
        challenge_hash: [2u8; 32],
        public_key: EmbeddedGroupAffine::generator() * EmbeddedFr::from(987_654_321u64),
        announcement: EmbeddedGroupAffine::generator() * EmbeddedFr::from(11u64),
        response: EmbeddedFr::from(0u64),
    };

    assert_eq!(
        issuance_challenge([1u8; 32], &proof).expect("challenge").as_le_bytes(),
        LACE_FIXED_CHALLENGE_LE
    );
}

#[test]
fn production_signing_uses_crypto_rng_and_redraws_zero_nonce() {
    let material = IssuerKeyMaterial::from_secret_scalar(EmbeddedFr::from(123_456_789u64));
    let vmr = verification_method_ref([0x11; 32], b"#key-assert").expect("vmr");
    let mut nonzero = [0u8; 64];
    nonzero[0] = 0x07;
    let mut rng = ScriptedCryptoRng::new(vec![[0u8; 64], nonzero]);

    let proof = sign_with_rng(&mut rng, &material, vmr, [1u8; 32], 123, [2u8; 32]).expect("sign");

    assert_eq!(proof.public_key, material.public_key());
    assert!(!proof.announcement.is_identity(), "zero nonce draw must be redrawn");
    let encoded = encode_detached_proof(&proof).expect("encode");
    assert_eq!(decode_detached_proof(&encoded).expect("decode"), proof);
    assert_eq!(
        verify_body_root([1u8; 32], &encoded).outcome,
        VerificationOutcome::Valid
    );
}

#[test]
fn rejects_trailing_noncanonical_oversized_and_identity_proof_inputs() {
    let proof = fixture(OXID_STANDALONE_PROOF_B64);
    let mut trailing = proof.clone();
    trailing.push(0);
    assert!(decode_detached_proof(&trailing).is_err());

    let mut noncanonical = proof.clone();
    let last = noncanonical.last_mut().expect("byte");
    *last = 0;
    assert!(decode_detached_proof(&noncanonical).is_err());

    assert!(decode_detached_proof(&vec![0u8; 2048]).is_err());

    let identity = encode_mcv1(&[vec![], vec![], vec![], vec![], vec![], vec![], vec![], vec![], vec![1]]);
    assert!(decode_detached_proof(&identity).is_err());
}

struct ScriptedCryptoRng {
    draws: Vec<[u8; 64]>,
}

impl ScriptedCryptoRng {
    fn new(draws: Vec<[u8; 64]>) -> Self {
        Self {
            draws: draws.into_iter().rev().collect(),
        }
    }
}

impl RngCore for ScriptedCryptoRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let draw = self.draws.pop().expect("scripted draw");
        assert_eq!(dest.len(), 64, "nonce draw must consume 64 bytes");
        dest.copy_from_slice(&draw);
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for ScriptedCryptoRng {}

fn encode_mcv1(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(b"MCV1");
    output.extend_from_slice(&(chunks.len() as u32).to_be_bytes());
    for chunk in chunks {
        output.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        output.extend_from_slice(chunk);
    }
    output
}
