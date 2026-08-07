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

//! Verification-method addressing for holder bindings — the shared seam
//! the Midnight VC stack consumes from the DID layer (issue #6).
//!
//! Ports the reference semantics from
//! `midnight-verifiable-credentials/packages/components/adapters/offchain-did/src/offchain-did-holder-binding.ts`:
//!
//! - **Domain-separated method-id hashing**: `SHA-256(domain ‖ NUL ‖ fragment)`
//!   with the `midnight:offchain:holder-method-id:v1` context tag. The NUL
//!   separator prevents collisions between this hash context and any
//!   future concatenation-based context.
//! - **Method-reference normalization**: accepts `#fragment`, a full DID
//!   URL bound to the resolved DID, or a bare fragment identifier —
//!   always yielding the canonical `#fragment` form the hash consumes.
//!
//! The [`hash_domains`] submodule owns the domain-tag vocabulary so the
//! DID- and VC-layer tags stay collision-free by construction.

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Domain-separation tags for off-chain SHA-256 hash contexts.
///
/// Tags are namespaced `midnight:<layer>:<purpose>:v<N>`; a new purpose
/// or a breaking change to an existing context gets a new tag, never a
/// silent reuse. In-circuit `persistentHash`/`transientHash` contexts
/// (e.g. `midnight:vc:issuance`) are owned by the Compact layer and are
/// intentionally NOT redefined here.
pub mod hash_domains {
    /// Off-chain DID holder method-id hash context (v1).
    pub const OFFCHAIN_DID_METHOD_ID_V1: &str = "midnight:offchain:holder-method-id:v1";
}

/// Errors from method-reference normalization.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MethodReferenceError {
    /// A `#`-prefixed reference was empty or contained a second `#`.
    #[error("method reference must contain exactly one non-empty fragment")]
    MalformedFragment,
    /// A DID-URL reference had no `#fragment` part.
    #[error("method reference must include a fragment")]
    MissingFragment,
    /// A DID-URL reference contained more than one `#`.
    #[error("method reference must contain only one fragment separator")]
    MultipleFragments,
    /// A DID-URL reference had an empty fragment.
    #[error("method reference must include a non-empty fragment")]
    EmptyFragment,
    /// A DID-URL reference belongs to a different DID than the resolved one.
    #[error("method reference must belong to the resolved DID")]
    ForeignDid,
    /// A bare reference contained a `#` in a non-prefix position.
    #[error("method reference must be a fragment, a full DID URL, or a bare fragment identifier")]
    AmbiguousReference,
}

/// `SHA-256(domain ‖ NUL ‖ payload)` — the generic domain-separated
/// hash every off-chain tag in [`hash_domains`] uses.
pub fn domain_separated_sha256(domain: &str, payload: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(payload);
    hasher.finalize().into()
}

/// Hash a **normalized** method-reference fragment (`#key-1` form) into
/// the 32-byte holder method-id the VC holder-binding circuits consume.
///
/// Mirrors the TS `hashOffchainDIDMethodId`.
pub fn hash_offchain_did_method_id(normalized_fragment: &str) -> [u8; 32] {
    domain_separated_sha256(hash_domains::OFFCHAIN_DID_METHOD_ID_V1, normalized_fragment.as_bytes())
}

/// Normalize a verification-method reference to canonical `#fragment`
/// form. Mirrors the TS `normalizeOffchainDIDMethodReference`:
///
/// - `#key-1` → accepted as-is (must contain exactly one `#`).
/// - `did:…#key-1` → the DID part must equal `did`; yields `#key-1`.
/// - `key-1` (bare, no `#`) → yields `#key-1`.
/// - anything else → error.
pub fn normalize_offchain_did_method_reference(
    method_reference: &str,
    did: &str,
) -> Result<String, MethodReferenceError> {
    if let Some(rest) = method_reference.strip_prefix('#') {
        if rest.is_empty() || rest.contains('#') {
            return Err(MethodReferenceError::MalformedFragment);
        }
        return Ok(method_reference.to_string());
    }
    if method_reference.starts_with("did:") {
        let (subject, fragment) = split_did_fragment(method_reference)?;
        if subject != did {
            return Err(MethodReferenceError::ForeignDid);
        }
        return Ok(format!("#{fragment}"));
    }
    if method_reference.contains('#') {
        return Err(MethodReferenceError::AmbiguousReference);
    }
    Ok(format!("#{method_reference}"))
}

/// Normalize then hash in one step — the common VC-adapter call shape.
pub fn method_reference_to_method_id(method_reference: &str, did: &str) -> Result<[u8; 32], MethodReferenceError> {
    let normalized = normalize_offchain_did_method_reference(method_reference, did)?;
    Ok(hash_offchain_did_method_id(&normalized))
}

fn split_did_fragment(method_reference: &str) -> Result<(&str, &str), MethodReferenceError> {
    let first = method_reference
        .find('#')
        .ok_or(MethodReferenceError::MissingFragment)?;
    let last = method_reference.rfind('#').expect("find succeeded");
    if first != last {
        return Err(MethodReferenceError::MultipleFragments);
    }
    let (subject, fragment_with_hash) = method_reference.split_at(first);
    let fragment = &fragment_with_hash[1..];
    if fragment.is_empty() {
        return Err(MethodReferenceError::EmptyFragment);
    }
    Ok((subject, fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DID: &str = "did:midnight:offchain:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    /// Golden vector computed from the TS reference implementation:
    /// `hashOffchainDIDMethodId("#holder-key-1")` in
    /// offchain-did-holder-binding.ts.
    #[test]
    fn method_id_hash_matches_ts_golden_vector() {
        let got = hash_offchain_did_method_id("#holder-key-1");
        assert_eq!(
            hex::encode(got),
            "318bfc180d719ef23b4e86f88135482611c3e0c828151613da04421700b971ca"
        );
    }

    #[test]
    fn nul_separator_prevents_domain_bleed() {
        // Without the NUL, domain "a" + payload "bc" would collide with
        // domain "ab" + payload "c".
        assert_ne!(domain_separated_sha256("a", b"bc"), domain_separated_sha256("ab", b"c"));
    }

    #[test]
    fn normalizes_fragment_forms() {
        assert_eq!(
            normalize_offchain_did_method_reference("#key-1", DID).unwrap(),
            "#key-1"
        );
        assert_eq!(normalize_offchain_did_method_reference("key-1", DID).unwrap(), "#key-1");
        assert_eq!(
            normalize_offchain_did_method_reference(&format!("{DID}#key-1"), DID).unwrap(),
            "#key-1"
        );
    }

    #[test]
    fn rejects_malformed_references() {
        use MethodReferenceError::*;
        assert_eq!(
            normalize_offchain_did_method_reference("#", DID).unwrap_err(),
            MalformedFragment
        );
        assert_eq!(
            normalize_offchain_did_method_reference("#a#b", DID).unwrap_err(),
            MalformedFragment
        );
        assert_eq!(
            normalize_offchain_did_method_reference("did:midnight:offchain:bbb#key-1", DID).unwrap_err(),
            ForeignDid
        );
        assert_eq!(
            normalize_offchain_did_method_reference("did:other:x", DID).unwrap_err(),
            MissingFragment
        );
        assert_eq!(
            normalize_offchain_did_method_reference(&format!("{DID}#a#b"), DID).unwrap_err(),
            MultipleFragments
        );
        assert_eq!(
            normalize_offchain_did_method_reference(&format!("{DID}#"), DID).unwrap_err(),
            EmptyFragment
        );
        assert_eq!(
            normalize_offchain_did_method_reference("weird#ref", DID).unwrap_err(),
            AmbiguousReference
        );
    }

    #[test]
    fn reference_to_method_id_composes() {
        let via_compose = method_reference_to_method_id("holder-key-1", DID).unwrap();
        let direct = hash_offchain_did_method_id("#holder-key-1");
        assert_eq!(via_compose, direct);
    }
}
