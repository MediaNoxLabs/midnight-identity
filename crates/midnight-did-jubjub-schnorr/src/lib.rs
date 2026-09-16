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
//! - **DID digest challenge**: `transientHash(ann_x, ann_y, pk_x, pk_y,
//!   digest…)` truncated **mod 2²⁴⁸** — matching `schnorrVerify`'s
//!   in-circuit `getSchnorrReduction` quotient check.
//! - **VC issuance-proof challenge primitive**: [`challenge_nonce_from_seed`]
//!   domain-separates nonce derivation with `:challenge` so a caller can feed
//!   an application-specific issuance-proof challenge into the same Schnorr
//!   response equation. It is intentionally not the full issuance-proof
//!   challenge construction, which belongs to the VC circuits.
//! - **Deterministic nonces**: `SHA-256("midnight-did:jubjub-schnorr:v1"
//!   ‖ seed ‖ digest) mod r` for the seed-based DID signing path; the
//!   challenge-domain primitive uses `SHA-256(domain ‖ ":challenge" ‖ seed
//!   ‖ random_seed) mod r`.
//! - **Encoding**: DID signatures are 96 bytes (`ann_x ‖ ann_y ‖ response`,
//!   each 32-byte big-endian). VC issuance-proof point helpers expose the
//!   separate 64-byte little-endian `x ‖ y` point shape.
//!
//! Compatibility table:
//!
//! | API / scheme | Challenge reduction | Nonce derivation | Wire shape | Interoperable with `midnight_transient_crypto::schnorr`? |
//! | --- | --- | --- | --- | --- |
//! | DID digest signature ([`sign_digest_from_seed`]) | mod 2²⁴⁸ truncation | v1 DID domain over seed + digest | 96-byte big-endian signature | No |
//! | VC issuance-proof primitive ([`challenge_nonce_from_seed`]) | caller-supplied challenge | v1 `:challenge` domain over seed + random seed | 64-byte little-endian points + 32-byte little-endian scalar | No |
//! | `midnight_transient_crypto::schnorr` | mod r | ledger implementation-specific | ledger implementation-specific | Only with itself |
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
    /// Text input exceeds the fixed 32-byte Compact field-fragment shape.
    #[error("text is {actual} bytes, exceeding the {max}-byte limit")]
    TextTooLong {
        /// Maximum accepted bytes.
        max: usize,
        /// Actual input length.
        actual: usize,
    },
    /// Hex input did not decode to exactly 32 bytes.
    #[error("hex input must decode to exactly 32 bytes")]
    BadHex32,
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

/// Portal-compatible `seedBytesToJubjubSecretScalar`: `SHA-256(seed32)`
/// interpreted as a big-endian integer and reduced into the Jubjub scalar
/// field. Unlike [`seed_to_secret_scalar`], this API requires the caller's
/// seed to already be exactly 32 bytes.
pub fn seed_bytes_to_secret_scalar(seed: &[u8; 32]) -> EmbeddedFr {
    hash_to_scalar(&Sha256::digest(seed))
}

/// Alias for [`seed_bytes_to_secret_scalar`] using the established consumer
/// helper name.
pub fn seed_bytes_to_jubjub_secret_scalar(seed: &[u8; 32]) -> EmbeddedFr {
    seed_bytes_to_secret_scalar(seed)
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

/// Derive the VC issuance-proof/challenge-domain nonce used by Oxid's
/// `sign_challenge` path: `SHA-256(NONCE_DOMAIN_V1 ‖ ":challenge" ‖
/// secret_seed32 ‖ random_seed32) mod r`.
///
/// This is a named primitive, not a DID digest challenge and not the full VC
/// issuance-proof challenge construction. Callers must compute their
/// application challenge separately and then form `s = k + c·x (mod r)`.
pub fn challenge_nonce_from_seed(secret_seed: &[u8; 32], random_seed: &[u8; 32]) -> EmbeddedFr {
    let mut preimage = Vec::with_capacity(NONCE_DOMAIN_V1.len() + b":challenge".len() + 64);
    preimage.extend_from_slice(NONCE_DOMAIN_V1.as_bytes());
    preimage.extend_from_slice(b":challenge");
    preimage.extend_from_slice(secret_seed);
    preimage.extend_from_slice(random_seed);
    hash_to_scalar(&Sha256::digest(preimage))
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
    rand::RngCore::fill_bytes(rng, &mut random);
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
    let x = field_from_be_bytes(&bytes[..32])?;
    let y = field_from_be_bytes(&bytes[32..64])?;
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

/// Draw a fresh random nonce from a cryptographically secure RNG, reducing 64
/// random bytes via the licensed curve implementation's wide-reduction API and
/// redrawing the negligible zero result instead of returning an identity nonce.
///
/// Schnorr nonces are secret signing material: callers must not supply
/// predictable or replayable RNGs.
pub fn random_nonce_scalar<R: rand::RngCore + rand::CryptoRng + ?Sized>(rng: &mut R) -> EmbeddedFr {
    loop {
        let mut wide = [0u8; 64];
        rng.fill_bytes(&mut wide);
        let nonce = scalar_from_wide_bytes(&wide);
        if nonce != EmbeddedFr::from(0u64) {
            return nonce;
        }
    }
}

/// Reduce 64 little-endian bytes into a Jubjub scalar using the curve
/// implementation's wide reduction.
pub fn scalar_from_wide_bytes(wide: &[u8; 64]) -> EmbeddedFr {
    EmbeddedFr(embedded::Scalar::from_bytes_wide(wide))
}

/// Convert a Jubjub scalar into a base field element via its canonical
/// little-endian bytes. This is lossless because the Jubjub scalar modulus is
/// smaller than the base field modulus.
pub fn field_from_scalar(scalar: &EmbeddedFr) -> Fr {
    Fr::from_le_bytes(&scalar.as_le_bytes()).expect("every Jubjub scalar fits in Fr")
}

/// Reduce a base field element's little-endian bytes into the Jubjub scalar
/// field.
pub fn scalar_from_field(field: &Fr) -> EmbeddedFr {
    reduce_le_bytes_to_scalar(&field.as_le_bytes())
}

/// Encode a base field element as 32 little-endian bytes.
pub fn field_to_bytes32_le(field: &Fr) -> [u8; 32] {
    let mut out = [0u8; 32];
    let le = field.as_le_bytes();
    let n = le.len().min(32);
    out[..n].copy_from_slice(&le[..n]);
    out
}

/// Encode a Jubjub point as 64 bytes `x ‖ y`, where both coordinates are
/// canonical 32-byte little-endian base-field encodings. This consumer proof
/// shape is deliberately distinct from the DID signature's 96-byte big-endian
/// encoding.
pub fn point_to_bytes64_le(point: &EmbeddedGroupAffine) -> Result<[u8; 64], SuiteError> {
    let (x, y) = coordinates(point)?;
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&field_to_bytes32_le(&x));
    out[32..].copy_from_slice(&field_to_bytes32_le(&y));
    Ok(out)
}

/// Alias for [`point_to_bytes64_le`] using the established consumer helper name.
pub fn point_to_bytes64(point: &EmbeddedGroupAffine) -> Result<[u8; 64], SuiteError> {
    point_to_bytes64_le(point)
}

/// Alias for [`field_to_bytes32_le`] using the established consumer helper name.
pub fn fr_to_bytes32(field: &Fr) -> [u8; 32] {
    field_to_bytes32_le(field)
}

/// Alias for [`field_from_scalar`] using the established consumer helper name.
pub fn fr_from_scalar(scalar: &EmbeddedFr) -> Fr {
    field_from_scalar(scalar)
}

/// UTF-8/text fragment padding helper: right-pad bytes with zeros to 32
/// bytes and fail closed instead of truncating when the input is longer.
pub fn pad_text_to_bytes32(bytes: &[u8]) -> Result<[u8; 32], SuiteError> {
    let mut out = [0u8; 32];
    let destination = out.get_mut(..bytes.len()).ok_or(SuiteError::TextTooLong {
        max: 32,
        actual: bytes.len(),
    })?;
    destination.copy_from_slice(bytes);
    Ok(out)
}

/// Decode exactly 64 hexadecimal characters into 32 bytes.
pub fn hex_decode_32(input: &str) -> Result<[u8; 32], SuiteError> {
    let decoded = hex::decode(input).map_err(|_| SuiteError::BadHex32)?;
    decoded.try_into().map_err(|_| SuiteError::BadHex32)
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
    let mut le = [0u8; 32];
    for (slot, byte) in le.iter_mut().zip(digest32.iter().take(32).rev()) {
        *slot = *byte;
    }
    reduce_le_bytes_to_scalar(&le)
}

fn reduce_le_bytes_to_scalar(bytes: &[u8]) -> EmbeddedFr {
    EmbeddedFr::from_le_bytes_wide(bytes).expect("callers pass at most 32 little-endian bytes")
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

fn field_from_be_bytes(be: &[u8]) -> Result<Fr, SuiteError> {
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
