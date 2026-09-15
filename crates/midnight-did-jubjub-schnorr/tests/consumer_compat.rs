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

//! Compatibility vectors independently reproduced from the issue #27 behavioral
//! contract and recorded read-only consumer baselines. Oxid's referenced source
//! is Apache-2.0; lace-id-portal remains an unlicensed factual/vector reference
//! only, so these tests assert independently written public behavior without
//! carrying source expression from that checkout.

use midnight_did_jubjub_schnorr::*;
use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine, Fr};

const TS_SEED: [u8; 32] = {
    let mut seed = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        seed[i] = (i + 1) as u8;
        i += 1;
    }
    seed
};

fn scalar_be_hex(scalar: &EmbeddedFr) -> String {
    let mut le = scalar.as_le_bytes();
    le.resize(32, 0);
    le.reverse();
    hex::encode(le)
}

#[test]
fn consumer_seed_scalar_and_public_key_bytes64_match_reproduced_vectors() {
    let scalar = seed_bytes_to_jubjub_secret_scalar(&TS_SEED);
    assert_eq!(
        scalar_be_hex(&scalar),
        "003cf13236b83e4b35ea71e39406dfdd00e489a68dae970a1e77b592a40ffa35"
    );
    assert_eq!(seed_bytes_to_secret_scalar(&TS_SEED), scalar);

    let public_key = derive_public_key(scalar);
    assert_eq!(
        hex::encode(point_to_bytes64(&public_key).expect("point bytes")),
        concat!(
            // TS parity coordinates rendered in the consumer proof point
            // byte order: 32 little-endian x bytes followed by 32 y bytes.
            "f41c213dc10ced0a582ca81fb354b0aee2cc3530fc294f09190b17cf7096cc10",
            "ddad992843449a75d5ca306c17a26c6df4547397e7fc409161d0db7016184c1f"
        )
    );
}

#[test]
fn consumer_padding_and_hex_helpers_fail_closed() {
    let padded = pad_text_to_bytes32(b"#key-assert").expect("pad");
    assert_eq!(&padded[..11], b"#key-assert");
    assert!(padded[11..].iter().all(|byte| *byte == 0));
    assert!(matches!(
        pad_text_to_bytes32(&[0x61; 33]),
        Err(SuiteError::TextTooLong { max: 32, actual: 33 })
    ));

    let decoded = hex_decode_32("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef").expect("hex");
    assert_eq!(decoded[0], 0x01);
    assert_eq!(decoded[31], 0xef);
    assert!(hex_decode_32("not hex").is_err());
}

#[test]
fn random_nonce_uses_all_64_bytes_and_retries_zero() {
    struct TwoDrawRng {
        calls: usize,
        second: [u8; 64],
    }

    impl rand::RngCore for TwoDrawRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            let source = if self.calls == 0 { [0; 64] } else { self.second };
            self.calls += 1;
            dest.copy_from_slice(&source[..dest.len()]);
        }

        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            self.fill_bytes(dest);
            Ok(())
        }
    }

    let mut wide = [0u8; 64];
    wide[0] = 1;
    wide[63] = 1;
    let reduced = scalar_from_wide_bytes(&wide);
    assert_eq!(
        scalar_be_hex(&reduced),
        "0995f2c3385c316d1da400aba48ba1dd132c6e5082ccf6e2821b2a17407f11fd"
    );
    assert_ne!(reduced, EmbeddedFr::from(1u64));

    let mut rng = TwoDrawRng { calls: 0, second: wide };
    assert_eq!(random_nonce_scalar(&mut rng), reduced);
    assert_eq!(rng.calls, 2, "zero first draw must be retried");
}

#[test]
fn oxid_fixed_seed_signature_vector_matches_apache2_reference() {
    let signature = sign_payload_from_seed(&[0x23; 32], b"Oxid holder statement").expect("signature");
    assert_eq!(
        hex::encode(encode_signature(&signature).expect("encode")),
        concat!(
            "583fe322acfa2db7c9328093c9c2fa83901fa81d81e6bab10af556ca91fc94bd",
            "519e689fcd0d1a7c988b864562a99be1774d88aa8bb69e79ecd1013ac9df0845",
            "08077115a06c82e6008f2f5496ce6d19e94c76d5909c9c1fa1da0d9f0e16dedb"
        )
    );
}

#[test]
fn oxid_challenge_nonce_uses_named_domain_extension() {
    let nonce = challenge_nonce_from_seed(&[0x23; 32], &[0x42; 32]);
    assert_eq!(
        scalar_be_hex(&nonce),
        "0695b41f06ab4eff086f79920631ba265531c9a79d21fe6355b30d35009e8940"
    );

    let announcement = EmbeddedGroupAffine::generator() * nonce;
    let public_key = derive_public_key(seed_bytes_to_jubjub_secret_scalar(&[0x23; 32]));
    assert_eq!(
        point_to_bytes64(&announcement).expect("alias"),
        point_to_bytes64_le(&announcement).expect("le")
    );
    assert_eq!(
        field_from_scalar(&nonce),
        Fr::from_le_bytes(&nonce.as_le_bytes()).expect("scalar fits")
    );
    assert_eq!(point_to_bytes64(&public_key).expect("pk").len(), 64);
}
