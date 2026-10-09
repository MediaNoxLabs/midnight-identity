// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

use midnight_did_jubjub_schnorr::{
    derive_public_key_from_seed, encode_signature, sign_authorization_digest_from_seed, verify_authorization_digest,
};
use midnight_transient_crypto::curve::Fr;

#[test]
fn full_field_authorization_digest_is_deterministic_and_verifiable() {
    let seed = [0x21; 32];
    let digest = [Fr::from(1_u64), Fr::from(2_u64), Fr::from(3_u64), Fr::from(4_u64)];
    let first = sign_authorization_digest_from_seed(&seed, &digest).expect("sign");
    let second = sign_authorization_digest_from_seed(&seed, &digest).expect("sign again");

    assert_eq!(
        encode_signature(&first).expect("encode"),
        encode_signature(&second).expect("encode")
    );
    assert!(verify_authorization_digest(
        &derive_public_key_from_seed(&seed),
        &digest,
        &first
    ));
}

#[test]
fn high_field_bits_are_not_truncated_before_signing() {
    let seed = [0x22; 32];
    let low = [Fr::from(1_u64), Fr::from(2_u64), Fr::from(3_u64), Fr::from(4_u64)];
    let mut high_bytes = [0_u8; 32];
    high_bytes[8] = 1;
    high_bytes[30] = 7;
    let high = [
        Fr::from_le_bytes(&high_bytes).expect("canonical field"),
        low[1],
        low[2],
        low[3],
    ];

    assert_ne!(
        encode_signature(&sign_authorization_digest_from_seed(&seed, &low).expect("low")).expect("encode"),
        encode_signature(&sign_authorization_digest_from_seed(&seed, &high).expect("high")).expect("encode")
    );
}
