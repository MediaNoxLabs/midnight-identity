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
//! Rust port of the testing slice of the standalone family repository's
//! `packages/midnight-verifiable-credential-digital-passport/src/testing/`
//! (pinned at tag `v0.1.0-rc1`): the claim-commitments fixture from
//! `credential-fixtures.ts` (same claim values, same
//! `sha256("opening:<field>")` openings) and the civil-date decomposition
//! helpers from `civil-date.ts` — everything the invariant-style smoke tests
//! need and nothing more. Commitments, the claim root, and the civil-date
//! witnesses are *derived* (through the generated circuits / the ported
//! Hinnant algorithm), never hardcoded: there are no golden outputs to go
//! stale.
//!
//! The `credential` / `presentation` bodies mirror upstream's fixture values
//! where they feed the age-predicate circuit (schema refs, disclosure flags,
//! `version: 1` request pinning); the signature/proof machinery around them
//! is deliberately omitted — the smoke tests exercise pure circuits, not the
//! signed protocol flow.

use midnight_vc_families::contract::digital_passport::{
    Credential, DigitalPassportCivilDate, DigitalPassportClaimCommitments, DigitalPassportClaimValues,
    DigitalPassportDisclosures, DigitalPassportOpenings, ExplicitHolderBinding, NoPublicClaims, NoStatusBinding,
    Presentation, SchemaRef, VerificationMethodRef, pure_circuits,
};
use sha2::{Digest, Sha256};

/// Claim values + openings + the commitments derived from them, plus the
/// age-predicate inputs, mirroring upstream's fixture tuple.
pub struct DigitalPassportFixture {
    /// The padded claim values (`DigitalPassportClaimValues` upstream).
    pub claim_values: DigitalPassportClaimValues,
    /// The per-claim hash openings (`DigitalPassportOpenings` upstream).
    pub openings: DigitalPassportOpenings,
    /// The commitments derived from `claim_values` × `openings` through the
    /// generated commitment circuits (`claimCommitments` upstream).
    pub claim_commitments: DigitalPassportClaimCommitments,
    /// The assembled credential body (`credential` upstream, minus its
    /// proof): the age predicate reads `claimCommitments` from it.
    pub credential: Credential,
    /// The assembled presentation body (`presentation` upstream, minus its
    /// proof): the age predicate reads the disclosure flags from it.
    pub presentation: Presentation,
    /// The evaluation day (`currentDay` upstream): `dateOfBirthDays` plus
    /// 25 flat years, comfortably past the default 18-year threshold.
    pub current_day: u32,
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

/// A deterministic `VerificationMethodRef` — upstream's `createSigner`
/// without the jubjub key pair (the smoke circuits never read the key).
fn verification_method_ref(label: &str) -> VerificationMethodRef {
    VerificationMethodRef {
        didContractAddress: midnight_vc_families::contract::digital_passport::ContractAddress {
            bytes: sha256(&format!("contract:{label}")),
        },
        methodId: pad_text(&format!("#{label}-key-1")),
    }
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

    let schema = SchemaRef {
        packageId: pad_text::<32>("midnight:vc:digital-passport"),
        schemaId: pad_text::<32>("digital-passport:v1"),
        majorVersion: 1,
        minorVersion: 0,
    };
    let issuer_verification_method_ref = verification_method_ref("issuer");
    let holder_binding = ExplicitHolderBinding {
        holderVerificationMethodRef: verification_method_ref("holder"),
    };

    let credential = Credential {
        version: 1,
        schema: schema.clone(),
        issuerVerificationMethodRef: issuer_verification_method_ref.clone(),
        holderBinding: holder_binding.clone(),
        statusBinding: NoStatusBinding {},
        issuedAt: 10_000,
        hasExpiration: true,
        expiresAt: 20_000,
        claims: NoPublicClaims {},
        claimRoot: pure_circuits::digital_passport_claim_root(claim_commitments.clone()).expect("claim root computes"),
        claimCommitments: claim_commitments.clone(),
    };

    let presentation = Presentation {
        version: 1,
        schema,
        credentialClaimRoot: credential.claimRoot,
        issuerVerificationMethodRef: issuer_verification_method_ref,
        holderBinding: holder_binding,
        disclosed: DigitalPassportDisclosures {
            revealFirstName: false,
            firstNameValuePadded: [0u8; 64],
            firstNameOpening: [0u8; 32],
            revealLastName: true,
            lastNameValuePadded: claim_values.lastNameValuePadded,
            lastNameOpening: openings.lastNameOpening,
            proveAgeOverThreshold: true,
            ageThresholdYears: 18,
            revealDocumentNumber: false,
            documentNumberValue: [0u8; 32],
            documentNumberOpening: [0u8; 32],
            revealIssuingState: false,
            issuingStateValue: [0u8; 32],
            issuingStateOpening: [0u8; 32],
        },
    };

    DigitalPassportFixture {
        current_day: 3650 + 365 * 25,
        claim_values,
        openings,
        claim_commitments,
        credential,
        presentation,
    }
}

/// Howard Hinnant's `days_from_civil`: the proleptic-Gregorian day number
/// (day 0 = 1970-01-01) of a valid civil date — upstream's
/// `epochDaysFromCivil`.
pub fn epoch_days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year_adjusted = if month <= 2 { year - 1 } else { year };
    let era = year_adjusted.div_euclid(400);
    let year_of_era = year_adjusted - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

/// The circuit-facing decomposition of a Unix-epoch day number: its unique
/// proleptic-Gregorian civil date plus the range-checked quotient witnesses
/// the circuit re-derives the day number from — upstream's
/// `civilDateFromEpochDays`.
pub fn civil_date_from_epoch_days(epoch_days: u32) -> DigitalPassportCivilDate {
    let z = i64::from(epoch_days) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = month_position + if month_position < 10 { 3 } else { -9 };
    let civil_year = year + if month <= 2 { 1 } else { 0 };
    let year_adjusted = if month <= 2 { civil_year - 1 } else { civil_year };
    let shifted_month = if month >= 3 { month - 3 } else { month + 9 };

    DigitalPassportCivilDate {
        year: civil_year as u32,
        month: month as u32,
        day: day as u32,
        yearAdjustedQuotient4: year_adjusted.div_euclid(4) as u32,
        yearAdjustedQuotient100: year_adjusted.div_euclid(100) as u32,
        yearAdjustedQuotient400: year_adjusted.div_euclid(400) as u32,
        marchBasedMonthDayOffset: ((153 * shifted_month + 2) / 5) as u32,
    }
}
