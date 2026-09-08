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

//! Invariant-style smoke tests for the generated digital-passport circuits.
//!
//! Ported from upstream's `src/test/claim-root.test.ts` (same deterministic
//! fixture): the circuits *compute* — outputs are deterministic, of the
//! documented `Bytes<32>` length, and sensitive to altered inputs. No golden
//! outputs are frozen; codegen drift is the codegen gate's job.

#![cfg(feature = "digital-passport")]

mod support;

use midnight_vc_families::contract::digital_passport::pure_circuits;
use support::create_digital_passport_fixture;

/// Claim root: deterministic, 32 bytes, and a different root when any one
/// commitment changes (upstream: "commits each claim field through a
/// domain-separated claim root").
#[test]
fn claim_root_is_deterministic_sized_and_alteration_sensitive() {
    let fixture = create_digital_passport_fixture();

    let root1 =
        pure_circuits::digital_passport_claim_root(fixture.claim_commitments.clone()).expect("claim root computes");
    let root2 = pure_circuits::digital_passport_claim_root(fixture.claim_commitments.clone())
        .expect("claim root computes again");

    assert_eq!(root1.len(), 32, "claim root is Bytes<32>");
    assert_eq!(root1, root2, "same fixture hashes to the same root");

    let mut altered = fixture.claim_commitments.clone();
    altered.dateOfBirthCommitment = [99u8; 32];
    let altered_root = pure_circuits::digital_passport_claim_root(altered).expect("altered claim root computes");
    assert_ne!(altered_root, root1, "altering one commitment changes the root");
}

/// Per-field commitments: re-deriving from the fixture's values × openings
/// reproduces the fixture's commitments, and a different opening yields a
/// different commitment (upstream: "produces deterministic commitments for
/// each field").
#[test]
fn claim_commitments_are_deterministic_and_opening_sensitive() {
    let fixture = create_digital_passport_fixture();

    let first_name = pure_circuits::first_name_commitment(
        fixture.claim_values.firstNameValuePadded,
        fixture.openings.firstNameOpening,
    )
    .expect("firstNameCommitment computes");
    let last_name = pure_circuits::last_name_commitment(
        fixture.claim_values.lastNameValuePadded,
        fixture.openings.lastNameOpening,
    )
    .expect("lastNameCommitment computes");
    let date_of_birth = pure_circuits::date_of_birth_commitment(
        fixture.claim_values.dateOfBirthDays,
        fixture.openings.dateOfBirthOpening,
    )
    .expect("dateOfBirthCommitment computes");
    let document_number = pure_circuits::document_number_commitment(
        fixture.claim_values.documentNumberValue,
        fixture.openings.documentNumberOpening,
    )
    .expect("documentNumberCommitment computes");
    let issuing_state = pure_circuits::issuing_state_commitment(
        fixture.claim_values.issuingStateValue,
        fixture.openings.issuingStateOpening,
    )
    .expect("issuingStateCommitment computes");

    assert_eq!(first_name, fixture.claim_commitments.firstNameCommitment);
    assert_eq!(last_name, fixture.claim_commitments.lastNameCommitment);
    assert_eq!(date_of_birth, fixture.claim_commitments.dateOfBirthCommitment);
    assert_eq!(document_number, fixture.claim_commitments.documentNumberCommitment);
    assert_eq!(issuing_state, fixture.claim_commitments.issuingStateCommitment);

    let different_opening = [42u8; 32];
    let first_name_other_opening =
        pure_circuits::first_name_commitment(fixture.claim_values.firstNameValuePadded, different_opening)
            .expect("firstNameCommitment computes with a different opening");
    assert_eq!(first_name.len(), 32, "commitments are Bytes<32>");
    assert_ne!(
        first_name_other_opening, first_name,
        "same value under a different opening commits differently"
    );
}

/// Sentinel for an absent document number: deterministic, 32 bytes, and
/// distinct from any real document-number commitment (upstream: "produces a
/// deterministic null commitment for absent documentNumber").
#[test]
fn document_number_null_commitment_is_a_deterministic_sentinel() {
    let null1 = pure_circuits::document_number_null_commitment().expect("null commitment computes");
    let null2 = pure_circuits::document_number_null_commitment().expect("null commitment computes again");

    assert_eq!(null1.len(), 32, "null commitment is Bytes<32>");
    assert_eq!(null1, null2);

    let fixture = create_digital_passport_fixture();
    assert_ne!(
        null1, fixture.claim_commitments.documentNumberCommitment,
        "the sentinel must not collide with a real document-number commitment"
    );
}
