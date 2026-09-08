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

//! Deterministic digital-passport test fixture.
//!
//! Rust port of the claim-commitments slice of the vendored submodule's
//! `packages/prototypes/credential-families/digital-passport/src/testing/credential-fixtures.ts`
//! (same claim values, same `sha256("opening:<field>")` openings) — everything
//! the invariant-style smoke tests need and nothing more. Commitments and the
//! claim root are *derived* through the generated circuits, never hardcoded:
//! there are no golden outputs to go stale.

use midnight_vc_families::contract::digital_passport::{
    DigitalPassportClaimCommitments, DigitalPassportClaimValues, DigitalPassportOpenings, pure_circuits,
};
use sha2::{Digest, Sha256};

/// Claim values + openings + the commitments derived from them, mirroring
/// upstream's fixture tuple.
pub struct DigitalPassportFixture {
    /// The padded claim values (`DigitalPassportClaimValues` upstream).
    pub claim_values: DigitalPassportClaimValues,
    /// The per-claim hash openings (`DigitalPassportOpenings` upstream).
    pub openings: DigitalPassportOpenings,
    /// The commitments derived from `claim_values` × `openings` through the
    /// generated commitment circuits (`claimCommitments` upstream).
    pub claim_commitments: DigitalPassportClaimCommitments,
}

/// `sha256(utf8(value))` — upstream's `sha256` helper.
fn sha256(value: &str) -> [u8; 32] {
    let digest: [u8; 32] = Sha256::digest(value.as_bytes()).into();
    digest
}

/// UTF-8 encode, then zero-pad (or truncate) to `N` bytes — upstream's
/// `padText`.
fn pad_text<const N: usize>(value: &str) -> [u8; N] {
    let bytes = value.as_bytes();
    let mut padded = [0u8; N];
    let n = bytes.len().min(N);
    padded[..n].copy_from_slice(&bytes[..n]);
    padded
}

/// Derive the five claim commitments from values × openings through the
/// generated circuits — the port of the fixture's `claimCommitments` block.
pub fn claim_commitments(
    claim_values: &DigitalPassportClaimValues,
    openings: &DigitalPassportOpenings,
) -> DigitalPassportClaimCommitments {
    DigitalPassportClaimCommitments {
        firstNameCommitment: pure_circuits::first_name_commitment(
            claim_values.firstNameValuePadded,
            openings.firstNameOpening,
        )
        .expect("firstNameCommitment computes"),
        lastNameCommitment: pure_circuits::last_name_commitment(
            claim_values.lastNameValuePadded,
            openings.lastNameOpening,
        )
        .expect("lastNameCommitment computes"),
        dateOfBirthCommitment: pure_circuits::date_of_birth_commitment(
            claim_values.dateOfBirthDays,
            openings.dateOfBirthOpening,
        )
        .expect("dateOfBirthCommitment computes"),
        documentNumberCommitment: pure_circuits::document_number_commitment(
            claim_values.documentNumberValue,
            openings.documentNumberOpening,
        )
        .expect("documentNumberCommitment computes"),
        issuingStateCommitment: pure_circuits::issuing_state_commitment(
            claim_values.issuingStateValue,
            openings.issuingStateOpening,
        )
        .expect("issuingStateCommitment computes"),
    }
}

/// The deterministic fixture: upstream's `createDigitalPassportFixture()`
/// with `hasDocumentNumber = true` (the default).
pub fn create_digital_passport_fixture() -> DigitalPassportFixture {
    let claim_values = DigitalPassportClaimValues {
        firstNameValuePadded: pad_text::<64>("Alice"),
        lastNameValuePadded: pad_text::<64>("Example"),
        dateOfBirthDays: 3650,
        documentNumberValue: pad_text::<32>("AB1234567"),
        issuingStateValue: pad_text::<32>("US"),
    };
    let openings = DigitalPassportOpenings {
        firstNameOpening: sha256("opening:first-name"),
        lastNameOpening: sha256("opening:last-name"),
        dateOfBirthOpening: sha256("opening:date-of-birth"),
        documentNumberOpening: sha256("opening:document-number"),
        issuingStateOpening: sha256("opening:issuing-state"),
    };
    let claim_commitments = claim_commitments(&claim_values, &openings);

    DigitalPassportFixture {
        claim_values,
        openings,
        claim_commitments,
    }
}
