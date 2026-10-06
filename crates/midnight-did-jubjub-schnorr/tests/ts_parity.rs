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

//! Cross-language golden vectors, generated 2026-08-07 from the TS
//! reference (`@midnight-ntwrk/midnight-did-jubjub-schnorr@0.5.0`,
//! upstream @ 42a8e4a) with:
//! seed = 0x01..0x20, payload =
//! b"midnight-identity jubjub-schnorr golden vector".

use midnight_did_jubjub_schnorr::*;

const SEED: [u8; 32] = {
    let mut s = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        s[i] = (i + 1) as u8;
        i += 1;
    }
    s
};
const PAYLOAD: &[u8] = b"midnight-identity jubjub-schnorr golden vector";

const TS_SK_HEX: &str = "003cf13236b83e4b35ea71e39406dfdd00e489a68dae970a1e77b592a40ffa35";
const TS_PK_X_HEX: &str = "10cc9670cf170b19094f29fc3035cce2aeb054b31fa82c580aed0cc13d211cf4";
const TS_PK_Y_HEX: &str = "1f4c181670dbd0619140fce7977354f46d6ca2176c30cad5759a44432899addd";
const TS_DIGEST: JubjubDigest = [
    0x2bdb0067176fd1bf,
    0xb0172636b6c91955,
    0xe28eed1304bc16d9,
    0xcbb1501030aa4576,
];
const TS_CHALLENGE_HEX: &str = "00ea4164e7c7915d9443d109b1b4ada096b5328543b42d97c1ce2994871d2dc8";
const TS_SIG96_HEX: &str = "02b4bfc039ddca33a2bc807a2df358682a81a6dd0db45eaf9567f00d000211460abff840b93c8fbc864111ba6009a31d227a9e04d44adcc6a44c0b103bb459da0603b2f0bc6eb850600cc297da66b157c88c53a731cfda0887153d531eabcd9c";

fn be_hex_32(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(bytes.as_ref())
}

#[test]
fn secret_scalar_matches_ts() {
    let sk = seed_to_secret_scalar(&SEED);
    let mut le = sk.as_le_bytes();
    le.resize(32, 0);
    le.reverse();
    assert_eq!(be_hex_32(le), TS_SK_HEX);
}

#[test]
fn public_key_matches_ts() {
    let pk = derive_public_key_from_seed(&SEED);
    let x = pk.x().expect("non-identity");
    let y = pk.y().expect("non-identity");
    let render = |fr: midnight_transient_crypto::curve::Fr| {
        let mut le = fr.as_le_bytes();
        le.resize(32, 0);
        le.reverse();
        hex::encode(le)
    };
    assert_eq!(render(x), TS_PK_X_HEX);
    assert_eq!(render(y), TS_PK_Y_HEX);
}

#[test]
fn digest_matches_ts() {
    assert_eq!(payload_to_digest(PAYLOAD), TS_DIGEST);
}

#[test]
fn deterministic_signature_matches_ts_bytes() {
    let sig = sign_payload_from_seed(&SEED, PAYLOAD).expect("sign");
    let encoded = encode_signature(&sig).expect("encode");
    assert_eq!(hex::encode(encoded), TS_SIG96_HEX);
}

#[test]
fn challenge_matches_ts() {
    let sig = sign_payload_from_seed(&SEED, PAYLOAD).expect("sign");
    let pk = derive_public_key_from_seed(&SEED);
    let challenge = compute_digest_challenge(&sig.announcement, &pk, &TS_DIGEST).expect("challenge");
    let mut le = challenge.as_le_bytes();
    le.resize(32, 0);
    le.reverse();
    assert_eq!(be_hex_32(le), TS_CHALLENGE_HEX);
}

#[test]
fn ts_signature_verifies_in_rust() {
    let ts_sig = decode_signature(&hex::decode(TS_SIG96_HEX).unwrap()).expect("decode");
    let pk = derive_public_key_from_seed(&SEED);
    assert!(verify_payload(&pk, PAYLOAD, &ts_sig));
}

#[test]
fn round_trip_encode_decode_verify() {
    let sig = sign_payload_from_seed(&SEED, PAYLOAD).expect("sign");
    let decoded = decode_signature(&encode_signature(&sig).expect("encode")).expect("decode");
    assert_eq!(decoded, sig);
    let pk = derive_public_key_from_seed(&SEED);
    assert!(verify_payload(&pk, PAYLOAD, &decoded));
}

#[test]
fn verification_rejects_wrong_inputs() {
    let sig = sign_payload_from_seed(&SEED, PAYLOAD).expect("sign");
    let pk = derive_public_key_from_seed(&SEED);
    assert!(!verify_payload(&pk, b"other payload", &sig));
    let other_pk = derive_public_key_from_seed(&[9u8; 32]);
    assert!(!verify_payload(&other_pk, PAYLOAD, &sig));
    // tampered response
    let mut bytes = encode_signature(&sig).expect("encode");
    bytes[95] ^= 1;
    let tampered = decode_signature(&bytes).expect("decode");
    assert!(!verify_payload(&pk, PAYLOAD, &tampered));
}

#[test]
fn decode_rejects_bad_inputs() {
    assert!(matches!(
        decode_signature(&[0u8; 95]),
        Err(SuiteError::BadSignatureLength)
    ));
    // x out of field (bit 255 set pushes the value past the ~2^254.86 modulus)
    let mut bytes = hex::decode(TS_SIG96_HEX).unwrap();
    bytes[0] |= 0x80;
    assert!(matches!(decode_signature(&bytes), Err(SuiteError::OutOfField)));
    // valid field elements that don't name a curve point
    let mut bytes = hex::decode(TS_SIG96_HEX).unwrap();
    bytes[31] ^= 0x01;
    assert!(matches!(decode_signature(&bytes), Err(SuiteError::NotOnCurve)));
}

#[test]
fn random_nonce_signing_verifies() {
    let mut rng = rand::rngs::OsRng;
    let sk = seed_to_secret_scalar(&SEED);
    let digest = payload_to_digest(PAYLOAD);
    let sig = sign_digest(&mut rng, sk, &digest).expect("sign");
    assert!(verify_digest(&derive_public_key(sk), &digest, &sig));
}

#[test]
fn short_and_long_seeds_follow_ts_padding() {
    // ensure32Bytes semantics: short → zero-pad right, long → truncate.
    let short = derive_public_key_from_seed(&[1, 2, 3]);
    let mut padded = [0u8; 32];
    padded[..3].copy_from_slice(&[1, 2, 3]);
    assert_eq!(short, derive_public_key_from_seed(&padded));

    let long: Vec<u8> = (1..=40).collect();
    assert_eq!(
        derive_public_key_from_seed(&long),
        derive_public_key_from_seed(&long[..32])
    );
}
