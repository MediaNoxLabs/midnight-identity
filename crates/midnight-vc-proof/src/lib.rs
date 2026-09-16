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

//! Stable VC issuance-proof surface over the generated Midnight Compact
//! runtime.
//!
//! This crate owns the hand-written boundary around the generated
//! `midnight-vc-runtime` issuance challenge. It does not reimplement Compact
//! persistent/transient hash semantics: [`issuance_challenge`] delegates to
//! `midnight_vc_runtime::contract::credentials::pure_circuits::issuance_proof_challenge`.
//! Digital-passport body-root helpers likewise use generated family bindings
//! for the checked-in parity fixture.
//!
//! # Non-swappable signature domains
//!
//! VC issuance proofs and DID digest signatures are distinct constructions.
//! DID signatures in `midnight-did-jubjub-schnorr` challenge a SHA-256 digest
//! with the DID circuit's `schnorrVerify` reduction. VC issuance proofs
//! challenge `(bodyRoot, issuance context, signer method, createdAt,
//! challengeHash, publicKey, R)` through the generated VC Compact circuit. A
//! byte-valid proof in one domain must never be replayed in the other.
//!
//! # Future chain-neutral seam
//!
//! The public API is deliberately policy-neutral: callers pass bounded artifact
//! bytes or a precomputed body root and receive staged evidence. Trust policy,
//! status/revocation policy, transport, and generic VC/VP request orchestration
//! stay outside this crate so a future chain-neutral SDK adapter can wrap these
//! primitives without importing Midnight generated types into the SDK core.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

use midnight_did_jubjub_schnorr::{field_from_scalar, random_nonce_scalar, scalar_from_field};
use midnight_transient_crypto::curve::{EmbeddedFr, EmbeddedGroupAffine, Fr};
use midnight_vc_families::contract::digital_passport as passport;
use midnight_vc_runtime::contract::credentials as core;
use rand::{CryptoRng, RngCore};

/// Compact-value v1 detached proof magic.
pub const MCV1_MAGIC: &[u8; 4] = b"MCV1";

/// Fixed chunk count for the generic VC `Proof` Compact value.
pub const PROOF_CHUNKS: usize = 9;

/// Fixed chunk count for the digital-passport `Credential` Compact value.
pub const DIGITAL_PASSPORT_CREDENTIAL_CHUNKS: usize = 18;

/// Maximum detached proof container size accepted by this crate.
pub const MAX_DETACHED_PROOF_BYTES: usize = 1024;

/// Maximum credential container size accepted by the digital-passport helper.
pub const MAX_CREDENTIAL_BYTES: usize = 8192;

/// Maximum Compact-value nesting depth accepted by this crate.
pub const MAX_COMPACT_VALUE_DEPTH: usize = 1;

/// Verification method reference hashed into the issuance challenge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerificationMethodRef {
    /// DID contract address bytes.
    pub did_contract_address: [u8; 32],
    /// Method fragment bytes, UTF-8 right-padded by the caller's DID layer.
    pub method_id: [u8; 32],
}

/// Issuance proof payload carried as a detached Compact value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IssuanceProof {
    /// Signer verification method reference.
    pub signer: VerificationMethodRef,
    /// Proof creation timestamp, seconds since Unix epoch.
    pub created_at: u64,
    /// Holder/request challenge hash bound into the proof.
    pub challenge_hash: [u8; 32],
    /// Issuer public key.
    pub public_key: EmbeddedGroupAffine,
    /// Schnorr announcement point `R`.
    pub announcement: EmbeddedGroupAffine,
    /// Schnorr response scalar `s`.
    pub response: EmbeddedFr,
}

/// Signing key material derived by the caller's custody boundary.
#[derive(Clone)]
pub struct IssuerKeyMaterial {
    secret_scalar: EmbeddedFr,
    public_key: EmbeddedGroupAffine,
}

impl std::fmt::Debug for IssuerKeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IssuerKeyMaterial").finish_non_exhaustive()
    }
}

impl IssuerKeyMaterial {
    /// Construct key material from an already-derived Jubjub secret scalar.
    #[must_use]
    pub fn from_secret_scalar(secret_scalar: EmbeddedFr) -> Self {
        let public_key = EmbeddedGroupAffine::generator() * secret_scalar;
        Self {
            secret_scalar,
            public_key,
        }
    }

    /// Return the issuer public key.
    #[must_use]
    pub fn public_key(&self) -> EmbeddedGroupAffine {
        self.public_key
    }
}

/// High-level verification outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The proof is structurally sound and satisfies the Schnorr equation.
    Valid,
    /// Evidence was parsed but is invalid.
    Invalid,
    /// Verification could not complete because a required operation failed.
    Error,
}

/// Verification stage names reported in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStageName {
    /// Detached proof parsing and canonical byte checks.
    Parse,
    /// Body-root and proof structure checks.
    Structure,
    /// Generated Compact challenge construction.
    Challenge,
    /// Schnorr signature equation check.
    Signature,
}

/// Status for one verification stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStageStatus {
    /// Stage passed.
    Passed,
    /// Stage failed.
    Failed,
    /// Stage was not reached after an earlier failure.
    NotChecked,
}

/// One staged verification evidence item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationStage {
    /// Stage name.
    pub name: VerificationStageName,
    /// Stage status.
    pub status: VerificationStageStatus,
    /// Stable, non-secret failure reason.
    pub reason: Option<&'static str>,
}

/// Structured verification report; callers should not collapse this to a bare bool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerificationReport {
    /// Final outcome.
    pub outcome: VerificationOutcome,
    /// Ordered stage evidence.
    pub stages: Vec<VerificationStage>,
}

/// Errors from bounded parsing, signing, or generated challenge construction.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProofError {
    /// Container is not canonical compact-value-v1.
    #[error("unsupported or malformed compact-value-v1 container")]
    MalformedContainer,
    /// Container exceeds the configured byte cap.
    #[error("compact value exceeds byte limit")]
    ByteLimit,
    /// Container has an unexpected chunk count.
    #[error("unexpected compact value chunk count")]
    ChunkCount,
    /// A scalar, field, or point is not canonical.
    #[error("non-canonical field, scalar, or point")]
    NonCanonicalCrypto,
    /// Point is the identity.
    #[error("identity point is not accepted in issuance proofs")]
    IdentityPoint,
    /// Generated runtime challenge construction failed.
    #[error("generated issuance challenge construction failed")]
    ChallengeUnavailable,
    /// Method identifier exceeds the 32-byte Compact field-fragment shape.
    #[error("method identifier exceeds 32 bytes")]
    MethodIdTooLong,
}

/// Sign a VC issuance proof with a fresh nonce from a cryptographically secure RNG.
pub fn sign_with_rng<R: RngCore + CryptoRng + ?Sized>(
    rng: &mut R,
    material: &IssuerKeyMaterial,
    signer: VerificationMethodRef,
    body_root: [u8; 32],
    created_at: u64,
    challenge_hash: [u8; 32],
) -> Result<IssuanceProof, ProofError> {
    let nonce = random_nonce_scalar(rng);
    sign_with_nonce(material, signer, body_root, created_at, challenge_hash, nonce)
}

/// Compute the generated VC issuance challenge for a partial proof.
pub fn issuance_challenge(body_root: [u8; 32], proof: &IssuanceProof) -> Result<Fr, ProofError> {
    core::pure_circuits::issuance_proof_challenge(body_root, runtime_proof(proof, EmbeddedFr::from(0_u64)))
        .map_err(|_| ProofError::ChallengeUnavailable)
}

/// Encode a detached proof as canonical compact-value-v1 bytes.
pub fn encode_detached_proof(proof: &IssuanceProof) -> Result<Vec<u8>, ProofError> {
    let (pk_x, pk_y) = point_coordinates(&proof.public_key)?;
    let (r_x, r_y) = point_coordinates(&proof.announcement)?;
    Ok(encode_mcv1(&[
        canonical_chunk(&proof.signer.did_contract_address),
        canonical_chunk(&proof.signer.method_id),
        canonical_chunk(&proof.created_at.to_le_bytes()),
        canonical_chunk(&proof.challenge_hash),
        canonical_chunk(&field_bytes32_le(&pk_x)),
        canonical_chunk(&field_bytes32_le(&pk_y)),
        canonical_chunk(&field_bytes32_le(&r_x)),
        canonical_chunk(&field_bytes32_le(&r_y)),
        canonical_chunk(&proof.response.as_le_bytes()),
    ]))
}

/// Decode and strictly validate a detached compact-value-v1 issuance proof.
pub fn decode_detached_proof(bytes: &[u8]) -> Result<IssuanceProof, ProofError> {
    let chunks = parse_mcv1(bytes, PROOF_CHUNKS, MAX_DETACHED_PROOF_BYTES)?;
    Ok(IssuanceProof {
        signer: VerificationMethodRef {
            did_contract_address: fixed_32(chunks[0])?,
            method_id: fixed_32(chunks[1])?,
        },
        created_at: integer_u64(chunks[2])?,
        challenge_hash: fixed_32(chunks[3])?,
        public_key: point(chunks[4], chunks[5])?,
        announcement: point(chunks[6], chunks[7])?,
        response: scalar(chunks[8])?,
    })
}

/// Verify a detached proof against a precomputed credential body root.
pub fn verify_body_root(body_root: [u8; 32], detached_proof: &[u8]) -> VerificationReport {
    let proof = match decode_detached_proof(detached_proof) {
        Ok(proof) => proof,
        Err(_) => {
            return report(
                VerificationOutcome::Invalid,
                VerificationStageName::Parse,
                "detached_proof_malformed",
            );
        }
    };
    let challenge = match issuance_challenge(body_root, &proof) {
        Ok(challenge) => challenge,
        Err(_) => {
            return report(
                VerificationOutcome::Error,
                VerificationStageName::Challenge,
                "challenge_unavailable",
            );
        }
    };
    let c_scalar = scalar_from_field(&challenge);
    let lhs = EmbeddedGroupAffine::generator() * proof.response;
    let rhs = proof.announcement + proof.public_key * c_scalar;
    if lhs == rhs {
        valid_report()
    } else {
        report(
            VerificationOutcome::Invalid,
            VerificationStageName::Signature,
            "invalid_issuance_signature",
        )
    }
}

/// Parse a digital-passport credential and compute its generated body root.
pub fn digital_passport_body_root(credential_bytes: &[u8]) -> Result<[u8; 32], ProofError> {
    let credential = decode_digital_passport_credential(credential_bytes)?;
    passport::pure_circuits::digital_passport_credential_body_root(credential)
        .map_err(|_| ProofError::ChallengeUnavailable)
}

/// Verify a digital-passport credential/proof pair with generated body-root and challenge semantics.
pub fn verify_digital_passport(credential_bytes: &[u8], detached_proof: &[u8]) -> VerificationReport {
    let proof = match decode_detached_proof(detached_proof) {
        Ok(proof) => proof,
        Err(_) => {
            return report(
                VerificationOutcome::Invalid,
                VerificationStageName::Parse,
                "detached_proof_malformed",
            );
        }
    };
    let credential = match decode_digital_passport_credential(credential_bytes) {
        Ok(credential) => credential,
        Err(_) => {
            return report(
                VerificationOutcome::Invalid,
                VerificationStageName::Structure,
                "credential_malformed",
            );
        }
    };
    if credential.issuerVerificationMethodRef.didContractAddress.bytes != proof.signer.did_contract_address
        || credential.issuerVerificationMethodRef.methodId != proof.signer.method_id
    {
        return report(
            VerificationOutcome::Invalid,
            VerificationStageName::Structure,
            "issuer_method_mismatch",
        );
    }
    let body_root = match passport::pure_circuits::digital_passport_credential_body_root(credential.clone()) {
        Ok(body_root) => body_root,
        Err(_) => {
            return report(
                VerificationOutcome::Invalid,
                VerificationStageName::Structure,
                "credential_malformed",
            );
        }
    };
    match passport::pure_circuits::assert_valid_issuance_context_proof(body_root, passport_proof(&proof)) {
        Ok(()) => valid_report(),
        Err(_) => report(
            VerificationOutcome::Invalid,
            VerificationStageName::Signature,
            "invalid_issuance_signature",
        ),
    }
}

/// Build a verification method reference from a 32-byte contract address and fragment bytes.
pub fn verification_method_ref(
    did_contract_address: [u8; 32],
    method_fragment: &[u8],
) -> Result<VerificationMethodRef, ProofError> {
    if method_fragment.len() > 32 {
        return Err(ProofError::MethodIdTooLong);
    }
    let mut method_id = [0u8; 32];
    method_id[..method_fragment.len()].copy_from_slice(method_fragment);
    Ok(VerificationMethodRef {
        did_contract_address,
        method_id,
    })
}

fn sign_with_nonce(
    material: &IssuerKeyMaterial,
    signer: VerificationMethodRef,
    body_root: [u8; 32],
    created_at: u64,
    challenge_hash: [u8; 32],
    nonce: EmbeddedFr,
) -> Result<IssuanceProof, ProofError> {
    let announcement = EmbeddedGroupAffine::generator() * nonce;
    let partial = IssuanceProof {
        signer,
        created_at,
        challenge_hash,
        public_key: material.public_key,
        announcement,
        response: EmbeddedFr::from(0_u64),
    };
    let challenge = issuance_challenge(body_root, &partial)?;
    let response = nonce + scalar_from_field(&challenge) * material.secret_scalar;
    Ok(IssuanceProof { response, ..partial })
}

fn decode_digital_passport_credential(bytes: &[u8]) -> Result<passport::Credential, ProofError> {
    let chunks = parse_mcv1(bytes, DIGITAL_PASSPORT_CREDENTIAL_CHUNKS, MAX_CREDENTIAL_BYTES)?;
    Ok(passport::Credential {
        version: integer_u16(chunks[0])?,
        schema: passport::SchemaRef {
            packageId: fixed_32(chunks[1])?,
            schemaId: fixed_32(chunks[2])?,
            majorVersion: integer_u16(chunks[3])?,
            minorVersion: integer_u16(chunks[4])?,
        },
        issuerVerificationMethodRef: passport_vmr(chunks[5], chunks[6])?,
        holderBinding: passport::ExplicitHolderBinding {
            holderVerificationMethodRef: passport_vmr(chunks[7], chunks[8])?,
        },
        statusBinding: passport::NoStatusBinding {},
        issuedAt: integer_u64(chunks[9])?,
        hasExpiration: boolean(chunks[10])?,
        expiresAt: integer_u64(chunks[11])?,
        claims: passport::NoPublicClaims {},
        claimCommitments: passport::DigitalPassportClaimCommitments {
            firstNameCommitment: fixed_32(chunks[12])?,
            lastNameCommitment: fixed_32(chunks[13])?,
            dateOfBirthCommitment: fixed_32(chunks[14])?,
            documentNumberCommitment: fixed_32(chunks[15])?,
            issuingStateCommitment: fixed_32(chunks[16])?,
        },
        claimRoot: fixed_32(chunks[17])?,
    })
}

fn passport_vmr(address: &[u8], method: &[u8]) -> Result<passport::VerificationMethodRef, ProofError> {
    Ok(passport::VerificationMethodRef {
        didContractAddress: passport::ContractAddress {
            bytes: fixed_32(address)?,
        },
        methodId: fixed_32(method)?,
    })
}

fn passport_proof(proof: &IssuanceProof) -> passport::Proof {
    passport::Proof {
        signerVerificationMethodRef: passport::VerificationMethodRef {
            didContractAddress: passport::ContractAddress {
                bytes: proof.signer.did_contract_address,
            },
            methodId: proof.signer.method_id,
        },
        createdAt: proof.created_at,
        challengeHash: proof.challenge_hash,
        publicKey: proof.public_key,
        signature: passport::Signature {
            r: proof.announcement,
            s: field_from_scalar(&proof.response),
        },
    }
}

fn runtime_proof(proof: &IssuanceProof, response: EmbeddedFr) -> core::Proof {
    core::Proof {
        signerVerificationMethodRef: core::VerificationMethodRef {
            didContractAddress: core::ContractAddress {
                bytes: proof.signer.did_contract_address,
            },
            methodId: proof.signer.method_id,
        },
        createdAt: proof.created_at,
        challengeHash: proof.challenge_hash,
        publicKey: proof.public_key,
        signature: core::Signature {
            r: proof.announcement,
            s: field_from_scalar(&response),
        },
    }
}

fn parse_mcv1(bytes: &[u8], expected_chunks: usize, byte_limit: usize) -> Result<Vec<&[u8]>, ProofError> {
    if bytes.len() < 8 || bytes.len() > byte_limit || &bytes[..4] != MCV1_MAGIC {
        return Err(if bytes.len() > byte_limit {
            ProofError::ByteLimit
        } else {
            ProofError::MalformedContainer
        });
    }
    let count = u32::from_be_bytes(bytes[4..8].try_into().map_err(|_| ProofError::MalformedContainer)?) as usize;
    if count != expected_chunks {
        return Err(ProofError::ChunkCount);
    }
    let mut offset = 8usize;
    let mut chunks = Vec::with_capacity(count);
    for _ in 0..count {
        let length_end = offset
            .checked_add(4)
            .filter(|end| *end <= bytes.len())
            .ok_or(ProofError::MalformedContainer)?;
        let length = u32::from_be_bytes(
            bytes[offset..length_end]
                .try_into()
                .map_err(|_| ProofError::MalformedContainer)?,
        ) as usize;
        offset = length_end;
        let end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or(ProofError::MalformedContainer)?;
        let chunk = &bytes[offset..end];
        if chunk.last() == Some(&0) {
            return Err(ProofError::MalformedContainer);
        }
        chunks.push(chunk);
        offset = end;
    }
    if offset != bytes.len() {
        return Err(ProofError::MalformedContainer);
    }
    Ok(chunks)
}

fn fixed_32(bytes: &[u8]) -> Result<[u8; 32], ProofError> {
    if bytes.len() > 32 {
        return Err(ProofError::MalformedContainer);
    }
    let mut out = [0u8; 32];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(out)
}

fn integer_u64(bytes: &[u8]) -> Result<u64, ProofError> {
    if bytes.len() > 8 {
        return Err(ProofError::MalformedContainer);
    }
    let mut out = [0u8; 8];
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(out))
}

fn integer_u16(bytes: &[u8]) -> Result<u16, ProofError> {
    u16::try_from(integer_u64(bytes)?).map_err(|_| ProofError::MalformedContainer)
}

fn boolean(bytes: &[u8]) -> Result<bool, ProofError> {
    match bytes {
        [] => Ok(false),
        [1] => Ok(true),
        _ => Err(ProofError::MalformedContainer),
    }
}

fn field(bytes: &[u8]) -> Result<Fr, ProofError> {
    if bytes.len() > 32 {
        return Err(ProofError::NonCanonicalCrypto);
    }
    Fr::from_le_bytes(bytes).ok_or(ProofError::NonCanonicalCrypto)
}

fn scalar(bytes: &[u8]) -> Result<EmbeddedFr, ProofError> {
    if bytes.len() > 32 {
        return Err(ProofError::NonCanonicalCrypto);
    }
    EmbeddedFr::from_le_bytes(bytes).ok_or(ProofError::NonCanonicalCrypto)
}

fn point(x_bytes: &[u8], y_bytes: &[u8]) -> Result<EmbeddedGroupAffine, ProofError> {
    let x = field(x_bytes)?;
    let y = field(y_bytes)?;
    if x == Fr::from(0_u64) && y == Fr::from(0_u64) {
        return Err(ProofError::IdentityPoint);
    }
    let point = EmbeddedGroupAffine::new(x, y).ok_or(ProofError::NonCanonicalCrypto)?;
    if point.x() != Some(x) || point.y() != Some(y) || point.is_identity() {
        return Err(ProofError::NonCanonicalCrypto);
    }
    Ok(point)
}

fn point_coordinates(point: &EmbeddedGroupAffine) -> Result<(Fr, Fr), ProofError> {
    match (point.x(), point.y()) {
        (Some(x), Some(y)) if !point.is_identity() => Ok((x, y)),
        _ => Err(ProofError::IdentityPoint),
    }
}

fn field_bytes32_le(field: &Fr) -> [u8; 32] {
    let mut out = [0u8; 32];
    let bytes = field.as_le_bytes();
    out[..bytes.len().min(32)].copy_from_slice(&bytes[..bytes.len().min(32)]);
    out
}

fn canonical_chunk(bytes: &[u8]) -> Vec<u8> {
    let length = bytes.iter().rposition(|byte| *byte != 0).map_or(0, |index| index + 1);
    bytes[..length].to_vec()
}

fn encode_mcv1(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MCV1_MAGIC);
    out.extend_from_slice(&(chunks.len() as u32).to_be_bytes());
    for chunk in chunks {
        out.extend_from_slice(&(chunk.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk);
    }
    out
}

fn valid_report() -> VerificationReport {
    VerificationReport {
        outcome: VerificationOutcome::Valid,
        stages: [
            VerificationStageName::Parse,
            VerificationStageName::Structure,
            VerificationStageName::Challenge,
            VerificationStageName::Signature,
        ]
        .into_iter()
        .map(|name| VerificationStage {
            name,
            status: VerificationStageStatus::Passed,
            reason: None,
        })
        .collect(),
    }
}

fn report(outcome: VerificationOutcome, failed: VerificationStageName, reason: &'static str) -> VerificationReport {
    let mut reached_failure = false;
    VerificationReport {
        outcome,
        stages: [
            VerificationStageName::Parse,
            VerificationStageName::Structure,
            VerificationStageName::Challenge,
            VerificationStageName::Signature,
        ]
        .into_iter()
        .map(|name| {
            if reached_failure {
                VerificationStage {
                    name,
                    status: VerificationStageStatus::NotChecked,
                    reason: None,
                }
            } else if name == failed {
                reached_failure = true;
                VerificationStage {
                    name,
                    status: VerificationStageStatus::Failed,
                    reason: Some(reason),
                }
            } else {
                VerificationStage {
                    name,
                    status: VerificationStageStatus::Passed,
                    reason: None,
                }
            }
        })
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_fixed_nonce_helper_round_trips_for_internal_tests_only() {
        let material = IssuerKeyMaterial::from_secret_scalar(EmbeddedFr::from(123_456_789u64));
        let signer = verification_method_ref([0x11; 32], b"#key-assert").expect("vmr");
        let proof = sign_with_nonce(&material, signer, [1u8; 32], 123, [2u8; 32], EmbeddedFr::from(11u64))
            .expect("fixed nonce signing is available only inside this module");
        let encoded = encode_detached_proof(&proof).expect("canonical proof");
        assert_eq!(
            verify_body_root([1u8; 32], &encoded).outcome,
            VerificationOutcome::Valid
        );
    }
}
