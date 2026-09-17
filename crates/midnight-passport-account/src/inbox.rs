// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

#![allow(deprecated)]

//! InboxEntry v1 fixed-width codec and client-side encryption.
//!
//! Ported from `contract/src/wallet/inbox.ts` in the pinned Passport revision.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::Sha256;
use x25519_dalek::{PublicKey, SharedSecret, StaticSecret};

/// Fixed InboxEntry v1 container size.
pub const ENTRY_SIZE: usize = 192;
/// InboxEntry version byte.
pub const ENTRY_VERSION: u8 = 0x01;
/// X25519 + HKDF-SHA256 + AES-256-GCM suite byte.
pub const ENTRY_SUITE: u8 = 0x01;
const PLAINTEXT_SIZE: usize = 80;
const HKDF_INFO: &[u8] = b"midnight:custody:inbox:v1";

/// Plain shielded coin description carried inside an InboxEntry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlainCoin {
    /// Coin nonce.
    pub nonce: [u8; 32],
    /// Coin color.
    pub color: [u8; 32],
    /// Coin value encoded in the entry as unsigned 128-bit big-endian.
    pub value: u128,
}

/// X25519 account encryption keypair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncKeyPair {
    /// Raw 32-byte public key advertised by the account.
    pub public_key: [u8; 32],
    /// Raw 32-byte secret viewing capability.
    pub secret_key: [u8; 32],
}

/// Reasons an entry could not be opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OpenEntryError {
    /// Entry is not exactly 192 bytes.
    #[error("inbox entry length is not 192 bytes")]
    BadLength,
    /// Version or suite is unknown and must be skipped.
    #[error("unknown inbox entry version or suite")]
    UnsupportedVersionOrSuite,
    /// Authentication failed or the entry is not for this key.
    #[error("inbox entry authentication failed")]
    AuthenticationFailed,
    /// X25519 produced a non-contributory shared secret.
    #[error("non-contributory X25519 shared secret")]
    NonContributorySharedSecret,
}

/// Generate a fresh X25519 account encryption keypair.
pub fn generate_enc_key_pair<R: RngCore + rand::CryptoRng>(rng: &mut R) -> EncKeyPair {
    let secret = StaticSecret::random_from_rng(&mut *rng);
    let public = PublicKey::from(&secret);
    EncKeyPair {
        public_key: public.to_bytes(),
        secret_key: secret.to_bytes(),
    }
}

fn derive_aead_key(shared: &SharedSecret) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(&[]), shared.as_bytes());
    let mut key = [0u8; 32];
    hk.expand(HKDF_INFO, &mut key).expect("32-byte HKDF output is valid");
    key
}

fn encode_coin(coin: &PlainCoin) -> [u8; PLAINTEXT_SIZE] {
    let mut out = [0u8; PLAINTEXT_SIZE];
    out[..32].copy_from_slice(&coin.nonce);
    out[32..64].copy_from_slice(&coin.color);
    out[64..].copy_from_slice(&coin.value.to_be_bytes());
    out
}

fn decode_coin(buf: &[u8; PLAINTEXT_SIZE]) -> PlainCoin {
    PlainCoin {
        nonce: buf[..32].try_into().expect("slice length"),
        color: buf[32..64].try_into().expect("slice length"),
        value: u128::from_be_bytes(buf[64..].try_into().expect("slice length")),
    }
}

/// Seal a coin for an account encryption public key into a fixed 192-byte entry.
pub fn seal_inbox_entry<R: RngCore + rand::CryptoRng>(
    rng: &mut R,
    recipient_enc_key: &[u8; 32],
    coin: &PlainCoin,
) -> Result<[u8; ENTRY_SIZE], OpenEntryError> {
    let eph_secret = StaticSecret::random_from_rng(&mut *rng);
    let eph_public = PublicKey::from(&eph_secret);
    let shared = eph_secret.diffie_hellman(&PublicKey::from(*recipient_enc_key));
    if !shared.was_contributory() {
        return Err(OpenEntryError::NonContributorySharedSecret);
    }
    let key = derive_aead_key(&shared);
    let cipher = Aes256Gcm::new_from_slice(&key).expect("AES-256 key length");
    let mut nonce_bytes = [0u8; 12];
    rng.fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce_bytes),
            Payload {
                msg: &encode_coin(coin),
                aad: &[ENTRY_VERSION, ENTRY_SUITE],
            },
        )
        .expect("AES-GCM sealing should succeed");
    debug_assert_eq!(ciphertext.len(), PLAINTEXT_SIZE + 16);

    let mut entry = [0u8; ENTRY_SIZE];
    entry[0] = ENTRY_VERSION;
    entry[1] = ENTRY_SUITE;
    entry[2..34].copy_from_slice(&eph_public.to_bytes());
    entry[34..46].copy_from_slice(&nonce_bytes);
    entry[46..62].copy_from_slice(&ciphertext[PLAINTEXT_SIZE..]);
    entry[62..142].copy_from_slice(&ciphertext[..PLAINTEXT_SIZE]);
    Ok(entry)
}

/// Open one fixed InboxEntry. Unknown/invalid entries return an error so
/// discovery can skip them without aborting the whole walk.
pub fn open_inbox_entry(enc_secret_key: &[u8; 32], entry: &[u8]) -> Result<PlainCoin, OpenEntryError> {
    if entry.len() != ENTRY_SIZE {
        return Err(OpenEntryError::BadLength);
    }
    if entry[0] != ENTRY_VERSION || entry[1] != ENTRY_SUITE {
        return Err(OpenEntryError::UnsupportedVersionOrSuite);
    }
    let eph_pub: [u8; 32] = entry[2..34].try_into().expect("slice length");
    let nonce: [u8; 12] = entry[34..46].try_into().expect("slice length");
    let tag = &entry[46..62];
    let ct = &entry[62..142];
    let shared = StaticSecret::from(*enc_secret_key).diffie_hellman(&PublicKey::from(eph_pub));
    if !shared.was_contributory() {
        return Err(OpenEntryError::NonContributorySharedSecret);
    }
    let key = derive_aead_key(&shared);
    let cipher = Aes256Gcm::new_from_slice(&key).expect("AES-256 key length");
    let mut combined = Vec::with_capacity(PLAINTEXT_SIZE + 16);
    combined.extend_from_slice(ct);
    combined.extend_from_slice(tag);
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &combined,
                aad: &entry[..2],
            },
        )
        .map_err(|_| OpenEntryError::AuthenticationFailed)?;
    let pt: [u8; PLAINTEXT_SIZE] = plaintext.try_into().map_err(|_| OpenEntryError::AuthenticationFailed)?;
    Ok(decode_coin(&pt))
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    #[test]
    fn roundtrip_and_fixed_codec() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let keys = generate_enc_key_pair(&mut rng);
        let coin = PlainCoin {
            nonce: [1; 32],
            color: [2; 32],
            value: 500,
        };
        let entry = seal_inbox_entry(&mut rng, &keys.public_key, &coin).unwrap();
        assert_eq!(entry.len(), ENTRY_SIZE);
        assert_eq!(entry[0], ENTRY_VERSION);
        assert_eq!(entry[1], ENTRY_SUITE);
        assert!(entry[142..].iter().all(|b| *b == 0));
        assert_eq!(open_inbox_entry(&keys.secret_key, &entry).unwrap(), coin);
    }

    #[test]
    fn negative_version_length_and_authentication_cases() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        let keys = generate_enc_key_pair(&mut rng);
        let coin = PlainCoin {
            nonce: [3; 32],
            color: [4; 32],
            value: u128::MAX,
        };
        let mut entry = seal_inbox_entry(&mut rng, &keys.public_key, &coin).unwrap();
        assert_eq!(
            open_inbox_entry(&keys.secret_key, &entry[..191]),
            Err(OpenEntryError::BadLength)
        );
        entry[0] = 2;
        assert_eq!(
            open_inbox_entry(&keys.secret_key, &entry),
            Err(OpenEntryError::UnsupportedVersionOrSuite)
        );
        entry[0] = ENTRY_VERSION;
        entry[100] ^= 1;
        assert_eq!(
            open_inbox_entry(&keys.secret_key, &entry),
            Err(OpenEntryError::AuthenticationFailed)
        );
    }

    #[test]
    fn rejects_non_contributory_x25519_inputs() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(13);
        let keys = generate_enc_key_pair(&mut rng);
        let coin = PlainCoin {
            nonce: [5; 32],
            color: [6; 32],
            value: 7,
        };

        assert_eq!(
            seal_inbox_entry(&mut rng, &[0u8; 32], &coin),
            Err(OpenEntryError::NonContributorySharedSecret)
        );

        let mut entry = seal_inbox_entry(&mut rng, &keys.public_key, &coin).unwrap();
        entry[2..34].copy_from_slice(&[0u8; 32]);
        assert_eq!(
            open_inbox_entry(&keys.secret_key, &entry),
            Err(OpenEntryError::NonContributorySharedSecret)
        );
    }
}
