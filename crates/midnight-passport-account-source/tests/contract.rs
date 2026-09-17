// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use midnight_passport_account_source::*;
use sha2::{Digest, Sha256};

const EXPECTED_LEDGER_ENTRIES: &[&str] = &[
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

const EXPECTED_EXPORTED_CIRCUITS: &[&str] = &[
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

#[test]
fn source_is_the_authenticated_hardened_snapshot() {
    assert_eq!(CONTRACT_SOURCE.len(), CONTRACT_BYTES);
    assert_eq!(format!("{:x}", Sha256::digest(CONTRACT_SOURCE)), CONTRACT_SHA256);
    assert!(CONTRACT_SOURCE.contains("require_live_k256_key(pk);\n  assert(envelope <= 1, \"unknown envelope\");"));
    assert!(CONTRACT_SOURCE.contains(
        "derive_boot_commitment_with_jubjub(salt: Bytes<32>, pk: JubjubPoint): Bytes<32> {\n  assert(ecMul(pk, 8 as JubjubScalar) != ecMulGenerator(0 as JubjubScalar),\n         \"device key has small order\");"
    ));
    assert!(CONTRACT_SOURCE.contains("return persistentHash<[Bytes<32>, Bytes<32>, JubjubPoint]>("));
    assert!(
        !CONTRACT_SOURCE
            .lines()
            .map(str::trim)
            .any(|line| line == "return persistentHash<[Bytes<32>, Bytes<32>, JubjubPoint](")
    );
    assert!(CONTRACT_SOURCE.contains(
        "derive_boot_commitment_with_k256(\n  salt:     Bytes<32>,\n  pk:       Secp256k1Point,\n  envelope: Uint<8>,\n): Bytes<32> {\n  assert(envelope <= 1, \"unknown envelope\");"
    ));
    assert!(CONTRACT_SOURCE.contains(
        "derive_device_entry_with_jubjub(\n  self_addr: ContractAddress,\n  pk:        JubjubPoint,\n  epoch:     Uint<32>,\n  counter:   Uint<64>,\n): Bytes<32> {\n  assert(ecMul(pk, 8 as JubjubScalar) != ecMulGenerator(0 as JubjubScalar),\n         \"device key has small order\");"
    ));
    assert!(CONTRACT_SOURCE.contains(
        "derive_device_entry_with_k256(\n  self_addr: ContractAddress,\n  pk:        Secp256k1Point,\n  envelope:  Uint<8>,\n  epoch:     Uint<32>,\n  counter:   Uint<64>,\n): Bytes<32> {\n  assert(envelope <= 1, \"unknown envelope\");"
    ));
    assert!(CONTRACT_SOURCE.contains(
        "derive_grant_id_with_k256(\n  self_addr:   ContractAddress,\n  pk:          Secp256k1Point,\n  envelope:    Uint<8>,\n  origin_hash: Bytes<32>,\n  slot:        Uint<8>,\n): Bytes<32> {\n  require_live_k256_key(pk);\n  assert(envelope <= 1, \"unknown envelope\");"
    ));
    assert!(CONTRACT_SOURCE.contains(
        "derive_grant_id_with_jubjub(\n  self_addr:   ContractAddress,\n  pk:          JubjubPoint,\n  origin_hash: Bytes<32>,\n  slot:        Uint<8>,\n): Bytes<32> {\n  assert(ecMul(pk, 8 as JubjubScalar) != ecMulGenerator(0 as JubjubScalar),\n         \"grantee key has small order\");"
    ));
    assert_eq!(UPSTREAM_CONTRACT_SOURCE.len(), UPSTREAM_CONTRACT_BYTES);
    assert_eq!(
        format!("{:x}", Sha256::digest(UPSTREAM_CONTRACT_SOURCE)),
        UPSTREAM_CONTRACT_SHA256
    );
    assert!(
        !UPSTREAM_CONTRACT_SOURCE
            .contains("require_live_k256_key(pk);\n  assert(envelope <= 1, \"unknown envelope\");")
    );
    assert_eq!(SPEC_VERSION, 2);
    assert_eq!(UPSTREAM_REVISION.len(), 40);
    assert_eq!(STANDARDS, ["MIP-0012", "MIP-0013"]);
    assert_eq!(AUTHORIZATION_ARMS, ["jubjub-schnorr", "ecdsa-secp256k1"]);
    assert_eq!(NORMATIVE_AUTHORIZATION_ARM, "jubjub-schnorr");
    assert_eq!(INTERIM_AUTHORIZATION_ARM, "ecdsa-secp256k1");
}

#[test]
fn source_has_only_the_compact_standard_library_import() {
    let imports: Vec<_> = CONTRACT_SOURCE
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("import ") || line.starts_with("include "))
        .collect();
    assert_eq!(imports, ["import CompactStandardLibrary;"]);
}

#[test]
fn reviewed_public_surface_has_not_drifted() {
    let ledger_entries: Vec<_> = CONTRACT_SOURCE
        .lines()
        .filter_map(|line| line.strip_prefix("export ledger "))
        .map(|line| line.split_once(':').unwrap().0.trim())
        .collect();
    let circuits: Vec<_> = CONTRACT_SOURCE
        .lines()
        .filter_map(|line| {
            line.strip_prefix("export circuit ")
                .or_else(|| line.strip_prefix("export pure circuit "))
        })
        .map(|line| line.split_once('(').unwrap().0.trim())
        .collect();

    assert_eq!(ledger_entries, EXPECTED_LEDGER_ENTRIES);
    assert_eq!(circuits, EXPECTED_EXPORTED_CIRCUITS);
    assert_eq!(LEDGER_ENTRIES, EXPECTED_LEDGER_ENTRIES);
    assert_eq!(EXPORTED_CIRCUITS, EXPECTED_EXPORTED_CIRCUITS);
}

#[test]
fn manifest_matches_public_constants() {
    let manifest: serde_json::Value = serde_json::from_str(SOURCE_MANIFEST).unwrap();
    assert_eq!(manifest["package"], env!("CARGO_PKG_NAME"));
    assert_eq!(manifest["contract"]["path"], CONTRACT_PATH);
    assert_eq!(manifest["contract"]["bytes"], CONTRACT_BYTES);
    assert_eq!(manifest["contract"]["sha256"], CONTRACT_SHA256);
    assert_eq!(manifest["contract"]["specVersion"], SPEC_VERSION);
    assert_eq!(manifest["provenance"]["repository"], UPSTREAM_REPOSITORY);
    assert_eq!(manifest["provenance"]["revision"], UPSTREAM_REVISION);
    assert_eq!(manifest["provenance"]["path"], UPSTREAM_PATH);
    assert_eq!(manifest["provenance"]["byteIdentical"], false);
    assert_eq!(
        json_strings(&manifest["provenance"]["patches"]),
        [
            "downstream-jubjub-boot-small-order-guard",
            "downstream-jubjub-device-entry-small-order-guard",
            "downstream-k256-activation-envelope-guard",
            "downstream-k256-boot-envelope-guard",
            "downstream-k256-device-entry-envelope-guard",
            "downstream-k256-grant-id-envelope-guard",
            "downstream-k256-grant-id-live-key-guard",
            "downstream-jubjub-grant-id-small-order-guard",
        ]
    );
    assert_eq!(
        manifest["provenance"]["pristineSnapshot"]["path"],
        UPSTREAM_CONTRACT_PATH
    );
    assert_eq!(
        manifest["provenance"]["pristineSnapshot"]["bytes"],
        UPSTREAM_CONTRACT_BYTES
    );
    assert_eq!(
        manifest["provenance"]["pristineSnapshot"]["sha256"],
        UPSTREAM_CONTRACT_SHA256
    );
    assert_eq!(manifest["compatibility"]["ledgerLine"], LEDGER_LINE);
    assert_eq!(manifest["compatibility"]["compactCompiler"], COMPACT_COMPILER_VERSION);
    assert_eq!(manifest["compatibility"]["compactRuntime"], COMPACT_RUNTIME_VERSION);
    assert_eq!(manifest["compatibility"]["compactJs"], COMPACT_JS_VERSION);
    assert_eq!(manifest["compatibility"]["midnightJs"], MIDNIGHT_JS_VERSION);
    assert_eq!(json_strings(&manifest["standards"]), STANDARDS);
    assert_eq!(json_strings(&manifest["authorizationArms"]), AUTHORIZATION_ARMS);
    assert_eq!(json_strings(&manifest["knownLimitations"]), KNOWN_LIMITATIONS);
    assert_eq!(
        json_strings(&manifest["publicSurface"]["ledgerEntries"]),
        LEDGER_ENTRIES
    );
    assert_eq!(
        json_strings(&manifest["publicSurface"]["exportedCircuits"]),
        EXPORTED_CIRCUITS
    );
}

#[test]
fn package_payload_is_explicit_and_complete() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(cargo_toml.contains(
        "include                = [ \"src/**\", \"contract/**\", \"tests/**\", \"manifest.json\", \"README.md\" ]"
    ));
    assert_eq!(
        collect_files(&root),
        BTreeSet::from([
            "Cargo.toml".to_owned(),
            "README.md".to_owned(),
            "contract/account.compact".to_owned(),
            "contract/account.compact.license".to_owned(),
            "contract/account.upstream.compact".to_owned(),
            "manifest.json".to_owned(),
            "src/lib.rs".to_owned(),
            "tests/contract.rs".to_owned(),
        ])
    );
}

fn json_strings(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry.as_str().unwrap())
        .collect()
}

fn collect_files(root: &Path) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for directory in ["contract", "src", "tests"] {
        collect_files_under(root, &root.join(directory), &mut files);
    }
    for file in ["Cargo.toml", "README.md", "manifest.json"] {
        if root.join(file).is_file() {
            files.insert(file.to_owned());
        }
    }
    files
}

fn collect_files_under(root: &Path, directory: &Path, files: &mut BTreeSet<String>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_files_under(root, &path, files);
        } else {
            files.insert(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
        }
    }
}
