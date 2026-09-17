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

//! Authenticated source distribution for the Midnight Passport account-custody
//! reference contract.
//!
//! The crate contains no Ledger, Compact runtime, proving, generated-code, or
//! sibling-checkout dependency. Its metadata records the reviewed Ledger 9.1
//! cohort without mixing that dependency graph into this Ledger 8 workspace.

#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, clippy::all)]

/// Stable relative path of the hardened contract within the published crate.
pub const CONTRACT_PATH: &str = "contract/account.compact";

/// Account-custody Compact source with the downstream activation hardening patch documented in the manifest.
pub const CONTRACT_SOURCE: &str = include_str!("../contract/account.compact");

/// SHA-256 digest of [`CONTRACT_SOURCE`].
pub const CONTRACT_SHA256: &str = "40bb6943e92b71066b3f7415e00395adc5e0bd980121b4fb45c27af3e8af05f0";

/// Size in bytes of [`CONTRACT_SOURCE`].
pub const CONTRACT_BYTES: usize = 105_906;

/// Stable relative path of the pristine upstream snapshot within the published crate.
pub const UPSTREAM_CONTRACT_PATH: &str = "contract/account.upstream.compact";

/// Byte-identical account-custody Compact source from [`UPSTREAM_REVISION`].
pub const UPSTREAM_CONTRACT_SOURCE: &str = include_str!("../contract/account.upstream.compact");

/// SHA-256 digest of [`UPSTREAM_CONTRACT_SOURCE`].
pub const UPSTREAM_CONTRACT_SHA256: &str = "1064b1998a71b5ade418f136428d82721e6021d6d939ff2f12797383bb3f8d50";

/// Size in bytes of [`UPSTREAM_CONTRACT_SOURCE`].
pub const UPSTREAM_CONTRACT_BYTES: usize = 105_349;

/// Account-custody contract specification version exposed on ledger.
pub const SPEC_VERSION: u32 = 2;

/// Immutable source repository.
pub const UPSTREAM_REPOSITORY: &str = "https://github.com/midnightntwrk/passport";

/// Immutable source revision.
pub const UPSTREAM_REVISION: &str = "40072709e3f5ac5d9b89f9d92d02a4413cbe05cc";

/// Source path at [`UPSTREAM_REVISION`].
pub const UPSTREAM_PATH: &str = "contract/contracts/account.compact";

/// Standards implemented by the reference contract.
pub const STANDARDS: &[&str] = &["MIP-0012", "MIP-0013"];

/// Authorization arms present in the reference contract.
pub const AUTHORIZATION_ARMS: &[&str] = &["jubjub-schnorr", "ecdsa-secp256k1"];

/// Normative MIP-0013 authorization arm.
pub const NORMATIVE_AUTHORIZATION_ARM: &str = "jubjub-schnorr";

/// Interim, non-normative engineering authorization arm.
pub const INTERIM_AUTHORIZATION_ARM: &str = "ecdsa-secp256k1";

/// Reviewed Midnight Ledger compatibility line.
pub const LEDGER_LINE: &str = "9.1";

/// Reviewed Compact compiler version.
pub const COMPACT_COMPILER_VERSION: &str = "0.33.0-rc.2";

/// Reviewed Compact runtime version.
pub const COMPACT_RUNTIME_VERSION: &str = "0.18.0-rc.1";

/// Reviewed Compact JavaScript bindings version.
pub const COMPACT_JS_VERSION: &str = "2.5.5-rc.6";

/// Reviewed Midnight JavaScript package cohort.
pub const MIDNIGHT_JS_VERSION: &str = "5.0.0-beta.4";

/// Compatibility limitations that consumers must preserve when presenting this source.
pub const KNOWN_LIMITATIONS: &[&str] = &[
    "Recovery is specified but this source exposes no recovery circuit.",
    "The ecdsa-secp256k1 authorization arm is interim engineering, not normative MIP-0013.",
    "The full contract requires wave deployment under the reviewed Ledger 9 block limits.",
];

/// Reviewed exported ledger state names, in source order.
pub const LEDGER_ENTRIES: &[&str] = &[
    "round",
    "enc_key",
    "inbox",
    "inbox_count",
    "unshielded_balances",
    "spec_version",
    "devices",
    "device_epoch",
    "device_count",
    "auth_nonce",
    "boot",
    "booted",
    "grants",
    "grant_generation",
];

/// Reviewed exported circuit names, including pure helpers, in source order.
pub const EXPORTED_CIRCUITS: &[&str] = &[
    "activate_initial_device_with_jubjub",
    "activate_initial_device_with_k256",
    "derive_boot_commitment_with_jubjub",
    "derive_boot_commitment_with_k256",
    "derive_device_entry_with_jubjub",
    "derive_device_entry_with_k256",
    "envelope_digest",
    "compute_public_point_with_jubjub",
    "compute_public_point_with_k256",
    "challenge_withdraw_unshielded_with_jubjub",
    "challenge_withdraw_shielded_with_jubjub",
    "challenge_withdraw_shielded_to_contract_with_jubjub",
    "challenge_append_inbox_with_jubjub",
    "challenge_rotate_enc_key_with_jubjub",
    "challenge_add_device_with_jubjub",
    "challenge_remove_device_with_jubjub",
    "challenge_withdraw_unshielded_with_k256",
    "challenge_withdraw_shielded_with_k256",
    "challenge_withdraw_shielded_to_contract_with_k256",
    "challenge_append_inbox_with_k256",
    "challenge_rotate_enc_key_with_k256",
    "challenge_add_device_with_k256",
    "challenge_remove_device_with_k256",
    "deposit_unshielded",
    "withdraw_unshielded_with_jubjub",
    "withdraw_unshielded_with_k256",
    "deposit_shielded",
    "append_inbox_with_jubjub",
    "append_inbox_with_k256",
    "withdraw_shielded_with_jubjub",
    "withdraw_shielded_with_k256",
    "withdraw_shielded_to_contract_with_jubjub",
    "withdraw_shielded_to_contract_with_k256",
    "rotate_enc_key_with_jubjub",
    "rotate_enc_key_with_k256",
    "add_device_with_jubjub",
    "add_device_with_k256",
    "remove_device_with_jubjub",
    "remove_device_with_k256",
    "derive_grant_id_with_k256",
    "derive_grant_id_with_jubjub",
    "derive_grant_object_commit",
    "derive_grant_spent_commit",
    "derive_grant_rp_commit",
    "derive_grant_scope_digest",
    "challenge_withdraw_unshielded_with_grant_k256",
    "challenge_withdraw_shielded_with_grant_k256",
    "challenge_withdraw_shielded_to_contract_with_grant_k256",
    "challenge_withdraw_unshielded_with_grant_jubjub",
    "challenge_withdraw_shielded_with_grant_jubjub",
    "challenge_withdraw_shielded_to_contract_with_grant_jubjub",
    "challenge_issue_grant_with_k256",
    "challenge_revoke_grant_with_k256",
    "challenge_revoke_all_grants_with_k256",
    "challenge_issue_grant_with_jubjub",
    "challenge_revoke_grant_with_jubjub",
    "challenge_revoke_all_grants_with_jubjub",
    "withdraw_unshielded_with_grant_k256",
    "withdraw_shielded_with_grant_k256",
    "withdraw_shielded_to_contract_with_grant_k256",
    "withdraw_unshielded_with_grant_jubjub",
    "withdraw_shielded_with_grant_jubjub",
    "withdraw_shielded_to_contract_with_grant_jubjub",
    "issue_grant_with_k256",
    "revoke_grant_with_k256",
    "revoke_all_grants_with_k256",
    "issue_grant_with_jubjub",
    "revoke_grant_with_jubjub",
    "revoke_all_grants_with_jubjub",
];

/// Machine-readable source provenance and compatibility manifest.
pub const SOURCE_MANIFEST: &str = include_str!("../manifest.json");
