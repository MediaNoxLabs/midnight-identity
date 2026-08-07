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

//! Schnorr-over-Jubjub signature suite for the Midnight DID/VC stack —
//! the Rust port of `@midnight-ntwrk/midnight-did-jubjub-schnorr@0.5.0`
//! (issue #7, ADR 0009 split rules 1+2).
//!
//! This is a **thin suite layer over midnight-ledger primitives**
//! (`EmbeddedGroupAffine`, `EmbeddedFr`, `transient_hash`); no curve or
//! hash math is reimplemented. What the suite adds is the exact
//! conventions the `did.compact` / `vc.compact` circuits verify:
//!
//! - **Digest packing**: SHA-256 of the payload split into four
//!   big-endian `u64` limbs (`Vector<4, Field>` in-circuit).
//! - **Challenge**: `transientHash(ann_x, ann_y, pk_x, pk_y, digest…)`
//!   truncated **mod 2²⁴⁸** — matching `schnorrVerify`'s in-circuit
//!   `getSchnorrReduction` quotient check. ⚠️ This differs from
//!   `midnight_transient_crypto::schnorr` (ledger-8+), which reduces the
//!   same hash **mod r**; the two schemes are NOT interoperable — do not
//!   swap one for the other when the ledger pin advances.
//! - **Deterministic nonces**: `SHA-256("midnight-did:jubjub-schnorr:v1"
//!   ‖ seed ‖ digest) mod r` for the seed-based signing path.
//! - **Encoding**: 96-byte signatures (`ann_x ‖ ann_y ‖ response`,
//!   each 32-byte big-endian).
//!
//! Cross-language parity is pinned by golden vectors generated from the
//! TS reference (see `tests/ts_parity.rs`).

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine, Fr, embedded};
use midnight_transient_crypto::hash::transient_hash;
use sha2::{Digest, Sha256};

/// Nonce-derivation domain tag for the deterministic seed-based signing
/// path (must match the TS suite byte-for-byte).
pub const NONCE_DOMAIN_V1: &str = "midnight-did:jubjub-schnorr:v1";

/// Encoded signature length: `ann_x ‖ ann_y ‖ response`, 32 bytes each.
pub const SIGNATURE_LENGTH_BYTES: usize = 96;

/// A payload digest as four big-endian `u64` limbs of its SHA-256 —
/// the `Vector<4, Field>` shape the circuits consume.
pub type JubjubDigest = [u64; 4];

/// A Schnorr signature over Jubjub in suite form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JubjubSchnorrSignature {
    /// Announcement point `R = k·G`.
    pub announcement: EmbeddedGroupAffine,
    /// Response scalar `s = k + c·sk (mod r)`.
    pub response: EmbeddedFr,
}

/// Errors from decoding / signing.
#[derive(Debug, thiserror::Error)]
pub enum SuiteError {
    /// Encoded signature has the wrong length.
    #[error("Jubjub signature must be exactly {SIGNATURE_LENGTH_BYTES} bytes")]
    BadSignatureLength,
    /// A 32-byte field encoding names a value outside the field.
    #[error("encoded value is not a canonical field element")]
    OutOfField,
    /// Announcement coordinates do not name a curve point.
    #[error("announcement coordinates are not a Jubjub point")]
    NotOnCurve,
    /// The point is the identity (no affine coordinates to encode).
    #[error("identity point cannot be encoded")]
    IdentityPoint,
}

// ─────────────────────────────────────────────────────────────────────
// Key derivation
// ─────────────────────────────────────────────────────────────────────

/// `SHA-256(seed₃₂) mod r` — the TS `seedBytesToJubjubSecretScalar`.
///
/// Seeds shorter than 32 bytes are zero-padded on the right; longer
/// ones are truncated (TS `ensure32Bytes` semantics).
pub fn seed_to_secret_scalar(seed: &[u8]) -> EmbeddedFr {
    hash_to_scalar(&Sha256::digest(ensure_32_bytes(seed)))
}

/// Public key for a secret scalar: `pk = sk·G`.
pub fn derive_public_key(secret: EmbeddedFr) -> EmbeddedGroupAffine {
    EmbeddedGroupAffine::generator() * secret
}

/// Seed → public key in one step (TS `deriveJubjubPublicKeyFromSeed`).
pub fn derive_public_key_from_seed(seed: &[u8]) -> EmbeddedGroupAffine {
    derive_public_key(seed_to_secret_scalar(seed))
}

// ─────────────────────────────────────────────────────────────────────
// Digest + challenge
// ─────────────────────────────────────────────────────────────────────

/// SHA-256 the payload and split into four big-endian `u64` limbs
/// (TS `payloadToJubjubDigest`).
pub fn payload_to_digest(payload: &[u8]) -> JubjubDigest {
    let hash = Sha256::digest(payload);
    let limb = |i: usize| u64::from_be_bytes(hash[i * 8..(i + 1) * 8].try_into().expect("8 bytes"));
    [limb(0), limb(1), limb(2), limb(3)]
}

/// The suite challenge: `transientHash(ann_x, ann_y, pk_x, pk_y, d0..d3)`
/// truncated mod 2²⁴⁸ (TS `computeJubjubDigestChallenge`; in-circuit,
/// `schnorrVerify`'s `getSchnorrReduction` proves this truncation).
///
/// 2²⁴⁸ < r, so the truncated challenge injects losslessly into the
/// Jubjub scalar field.
pub fn compute_digest_challenge(
    announcement: &EmbeddedGroupAffine,
    public_key: &EmbeddedGroupAffine,
    digest: &JubjubDigest,
) -> Result<EmbeddedFr, SuiteError> {
    let (ann_x, ann_y) = coordinates(announcement)?;
    let (pk_x, pk_y) = coordinates(public_key)?;
    let preimage = [
        ann_x,
        ann_y,
        pk_x,
        pk_y,
        Fr::from(digest[0]),
        Fr::from(digest[1]),
        Fr::from(digest[2]),
        Fr::from(digest[3]),
    ];
    let c_full = transient_hash(&preimage);
    // mod 2^248 == keep the low 31 bytes of the little-endian repr.
    let mut le = c_full.as_le_bytes();
    le.resize(32, 0);
    le[31] = 0;
    Ok(EmbeddedFr::from_le_bytes(&le).expect("2^248-truncated value is < r"))
}

// ─────────────────────────────────────────────────────────────────────
// Signing
// ─────────────────────────────────────────────────────────────────────

/// Sign a digest with an explicit nonce seed (deterministic).
///
/// The nonce is `SHA-256(nonce_seed) mod r` — callers should build
/// `nonce_seed` with domain separation; [`sign_digest_from_seed`] does
/// the standard v1 construction.
pub fn sign_digest_with_nonce_seed(
    secret: EmbeddedFr,
    digest: &JubjubDigest,
    nonce_seed: &[u8],
) -> Result<JubjubSchnorrSignature, SuiteError> {
    let public_key = derive_public_key(secret);
    let nonce = hash_to_scalar(&Sha256::digest(nonce_seed));
    let announcement = EmbeddedGroupAffine::generator() * nonce;
    let challenge = compute_digest_challenge(&announcement, &public_key, digest)?;
    Ok(JubjubSchnorrSignature {
        announcement,
        response: nonce + challenge * secret,
    })
}

/// Sign a digest with fresh randomness (TS `signJubjubDigest` without a
/// nonce seed). DID-facing code should prefer the deterministic
/// [`sign_digest_from_seed`].
pub fn sign_digest<R: rand::Rng + rand::CryptoRng>(
    rng: &mut R,
    secret: EmbeddedFr,
    digest: &JubjubDigest,
) -> Result<JubjubSchnorrSignature, SuiteError> {
    let mut random = [0u8; 32];
    rng.fill_bytes(&mut random);
    let mut nonce_seed = Vec::with_capacity(32 + 32 + 128);
    nonce_seed.extend_from_slice(&scalar_be_bytes(&secret));
    nonce_seed.extend_from_slice(&random);
    nonce_seed.extend_from_slice(&serialize_digest(digest));
    sign_digest_with_nonce_seed(secret, digest, &nonce_seed)
}

/// The standard deterministic path (TS `signJubjubDigestFromSeed`):
/// nonce seed = `NONCE_DOMAIN_V1 ‖ seed₃₂ ‖ digest₄ₓ₃₂`.
pub fn sign_digest_from_seed(seed: &[u8], digest: &JubjubDigest) -> Result<JubjubSchnorrSignature, SuiteError> {
    let normalized = ensure_32_bytes(seed);
    let mut nonce_seed = Vec::with_capacity(NONCE_DOMAIN_V1.len() + 32 + 128);
    nonce_seed.extend_from_slice(NONCE_DOMAIN_V1.as_bytes());
    nonce_seed.extend_from_slice(&normalized);
    nonce_seed.extend_from_slice(&serialize_digest(digest));
    sign_digest_with_nonce_seed(seed_to_secret_scalar(seed), digest, &nonce_seed)
}

/// Hash the payload and sign it deterministically from the seed
/// (TS `signJubjubPayloadFromSeed`).
pub fn sign_payload_from_seed(seed: &[u8], payload: &[u8]) -> Result<JubjubSchnorrSignature, SuiteError> {
    sign_digest_from_seed(seed, &payload_to_digest(payload))
}

// ─────────────────────────────────────────────────────────────────────
// Verification
// ─────────────────────────────────────────────────────────────────────

/// Verify a digest signature (TS `verifyJubjubDigest`): recompute the
/// challenge and check `s·G == R + c·pk`.
pub fn verify_digest(
    public_key: &EmbeddedGroupAffine,
    digest: &JubjubDigest,
    signature: &JubjubSchnorrSignature,
) -> bool {
    let Ok(challenge) = compute_digest_challenge(&signature.announcement, public_key, digest) else {
        return false;
    };
    let lhs = EmbeddedGroupAffine::generator() * signature.response;
    let rhs = signature.announcement + *public_key * challenge;
    lhs == rhs
}

/// Hash the payload and verify (TS `verifyJubjubPayload`).
pub fn verify_payload(public_key: &EmbeddedGroupAffine, payload: &[u8], signature: &JubjubSchnorrSignature) -> bool {
    verify_digest(public_key, &payload_to_digest(payload), signature)
}

// ─────────────────────────────────────────────────────────────────────
// Encoding
// ─────────────────────────────────────────────────────────────────────

/// Encode as 96 bytes: `ann_x ‖ ann_y ‖ response` (32-byte big-endian
/// each; TS `encodeJubjubSignature`).
pub fn encode_signature(signature: &JubjubSchnorrSignature) -> Result<[u8; 96], SuiteError> {
    let (x, y) = coordinates(&signature.announcement)?;
    let mut out = [0u8; 96];
    out[..32].copy_from_slice(&fr_be_bytes(&x));
    out[32..64].copy_from_slice(&fr_be_bytes(&y));
    out[64..].copy_from_slice(&scalar_be_bytes(&signature.response));
    Ok(out)
}

/// Decode a 96-byte signature, validating the announcement is a curve
/// point (TS `decodeJubjubSignature`, plus on-curve validation the TS
/// version defers to verify time).
pub fn decode_signature(bytes: &[u8]) -> Result<JubjubSchnorrSignature, SuiteError> {
    if bytes.len() != SIGNATURE_LENGTH_BYTES {
        return Err(SuiteError::BadSignatureLength);
    }
    let x = fr_from_be_bytes(&bytes[..32])?;
    let y = fr_from_be_bytes(&bytes[32..64])?;
    let announcement = EmbeddedGroupAffine::new(x, y).ok_or(SuiteError::NotOnCurve)?;
    // `new` runs the input through cofactor clearing (`into_subgroup`),
    // which launders off-curve/off-subgroup inputs into *different*
    // points instead of rejecting them. A coordinate round-trip detects
    // that: only prime-order subgroup points survive unchanged.
    if announcement.x() != Some(x) || announcement.y() != Some(y) {
        return Err(SuiteError::NotOnCurve);
    }
    let mut response_le: Vec<u8> = bytes[64..96].to_vec();
    response_le.reverse();
    let response = EmbeddedFr::from_le_bytes(&response_le).ok_or(SuiteError::OutOfField)?;
    Ok(JubjubSchnorrSignature { announcement, response })
}

// ─────────────────────────────────────────────────────────────────────
// Internals
// ─────────────────────────────────────────────────────────────────────

fn ensure_32_bytes(seed: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let n = seed.len().min(32);
    out[..n].copy_from_slice(&seed[..n]);
    out
}

/// Interpret 32 hash bytes as a big-endian integer mod r
/// (TS `hashToScalar`).
fn hash_to_scalar(digest32: &[u8]) -> EmbeddedFr {
    let mut wide = [0u8; 64];
    // big-endian bytes → little-endian limbs in the low half.
    for (i, b) in digest32.iter().rev().enumerate() {
        wide[i] = *b;
    }
    EmbeddedFr(embedded::Scalar::from_bytes_wide(&wide))
}

fn coordinates(point: &EmbeddedGroupAffine) -> Result<(Fr, Fr), SuiteError> {
    match (point.x(), point.y()) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(SuiteError::IdentityPoint),
    }
}

fn fr_be_bytes(fr: &Fr) -> [u8; 32] {
    let mut le = fr.as_le_bytes();
    le.resize(32, 0);
    le.reverse();
    le.try_into().expect("32 bytes")
}

fn fr_from_be_bytes(be: &[u8]) -> Result<Fr, SuiteError> {
    let mut le: Vec<u8> = be.to_vec();
    le.reverse();
    Fr::from_le_bytes(&le).ok_or(SuiteError::OutOfField)
}

fn scalar_be_bytes(scalar: &EmbeddedFr) -> [u8; 32] {
    let mut le = scalar.as_le_bytes();
    le.resize(32, 0);
    le.reverse();
    le.try_into().expect("32 bytes")
}

fn serialize_digest(digest: &JubjubDigest) -> [u8; 128] {
    let mut out = [0u8; 128];
    for (i, limb) in digest.iter().enumerate() {
        // 32-byte big-endian rendering of each u64 limb (TS bigintTo32Be).
        out[i * 32 + 24..(i + 1) * 32].copy_from_slice(&limb.to_be_bytes());
    }
    out
}
