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

//! On-chain state → [`DidLedgerSnapshot`] decoding.
//!
//! This is the read half of the LiveBackend bridge (issue #4): given the
//! raw bytes the indexer returns for `contractAction(address){state}`,
//! deserialize the tagged `ContractState` and project the DID ledger
//! fields into the plain-data [`DidLedgerSnapshot`] the resolution layer
//! consumes.
//!
//! ## State layout
//!
//! did.compact 0.5.0 declares 19 ledger fields. `StateValue::Array` is
//! capped at 16 slots, so the on-chain layout (as produced by the TS
//! reference constructor and read by every generated accessor) chunks
//! them into an outer 2-slot array: `[0]` holds fields 0–3 and `[1]`
//! holds fields 4–18 (`f < 4 → [0][f]`, else `[1][f-4]`).
//!
//! Scalar cells are read through the generated [`crate::contract::ledger`]
//! accessors; the Map/Set fields have no generated accessors (the Rust
//! codegen emits scalar views only), so this module walks
//! `StateValue::Map` entries directly.
//!
//! ## Fidelity notes
//!
//! - `controller_public_key_hex` carries only the Jubjub **x** coordinate
//!   (little-endian hex): the snapshot type predates the 0.5.0 full-point
//!   model. Carrying the full point is tracked by issue #3.
//! - The ledger's `recoveryAuthorityPublicKey` has no snapshot field and
//!   is not decoded (also issue #3 territory).

use std::collections::BTreeMap;

use compact_runtime::{AlignedValue, ChargedState, ContractState, DefaultDB, JubjubPoint, StateValue, aligned_bytes};
use midnight_did_domain::did_document::{CurveType, KeyType, VerificationMethodType};

use crate::backend::BackendError;
use crate::contract::ledger;
use crate::contract_call::{
    DidLedgerSnapshot, JubjubPointHex, LedgerPublicKeyJwk, LedgerSchnorrJubjubVerificationMethod, LedgerService,
    LedgerVerificationMethod, NewJubjubPointHex,
};

/// Slot paths of the collection fields inside the `[1]` chunk
/// (field index − 4). Mirrors the generated accessors' `idx_at_index`
/// pairs; see the module docs for the chunking rule.
mod slot {
    pub const ALSO_KNOWN_AS: usize = 0;
    pub const VERIFICATION_METHODS: usize = 7;
    pub const SCHNORR_JUBJUB_VERIFICATION_METHODS: usize = 8;
    pub const AUTHENTICATION_RELATION: usize = 9;
    pub const ASSERTION_METHOD_RELATION: usize = 10;
    pub const KEY_AGREEMENT_RELATION: usize = 11;
    pub const CAPABILITY_INVOCATION_RELATION: usize = 12;
    pub const CAPABILITY_DELEGATION_RELATION: usize = 13;
    pub const SERVICES: usize = 14;
}

/// Deserialize the raw bytes the indexer returns for a contract's
/// `state` field into the contract's charged state.
///
/// The byte stream is self-describing (`midnight:contract-state[v6]:`
/// tag prefix), so a version mismatch with the pinned ledger crates
/// fails loudly rather than mis-decoding.
pub fn charged_state_from_bytes(bytes: &[u8]) -> Result<ChargedState<DefaultDB>, BackendError> {
    let contract_state: ContractState<DefaultDB> = midnight_serialize::tagged_deserialize(&mut &bytes[..])
        .map_err(|e| BackendError::Decode(format!("ContractState deserialize failed: {e}")))?;
    Ok(contract_state.data)
}

/// Project a charged contract state into a [`DidLedgerSnapshot`].
///
/// Scalars go through the generated `ledger()` accessors; Map/Set fields
/// are walked directly (no generated ADT views exist — see module docs).
pub fn decode_ledger_snapshot(state: &ChargedState<DefaultDB>) -> Result<DidLedgerSnapshot, BackendError> {
    let view = ledger(state);
    let chunks = StateChunks::from_state(state)?;

    // NOTE: not `view.id()`. The generated accessor decodes the cell via
    // `decode_via_field_repr::<ContractAddress>` ([u8;32] FIELD_SIZE = 2),
    // but a Bytes<32> cell is alignment-encoded as ONE 32-byte atom, so
    // that accessor can never succeed against a real cell. Read the raw
    // bytes directly. (Codegen follow-up tracked on the compact PR.)
    let id_hex = hex::encode(chunks.raw_bytes_cell(0, 3, "id")?);
    let controller_public_key_hex = jubjub_x_hex(
        &view
            .controller_public_key()
            .map_err(|e| decode_err("controllerPublicKey", &e))?,
    )?;

    let collections = CollectionSlots { inner: chunks.chunk1 };

    Ok(DidLedgerSnapshot {
        id_hex,
        active: view.active().map_err(|e| decode_err("active", &e))?,
        deactivated: view.deactivated().map_err(|e| decode_err("deactivated", &e))?,
        controller_public_key_hex,
        version: view.version().map_err(|e| decode_err("version", &e))?,
        operation_count: view.operation_count().map_err(|e| decode_err("operationCount", &e))?,
        contract_version: u64::from(view.contract_version().map_err(|e| decode_err("contractVersion", &e))?),
        created_ms: view.created().map_err(|e| decode_err("created", &e))?,
        updated_ms: view.updated().map_err(|e| decode_err("updated", &e))?,
        also_known_as: collections.string_set(slot::ALSO_KNOWN_AS, "alsoKnownAs")?,
        verification_methods: collections.verification_methods()?,
        schnorr_jubjub_verification_methods: collections.schnorr_jubjub_verification_methods()?,
        authentication_relation: collections.string_set(slot::AUTHENTICATION_RELATION, "authenticationRelation")?,
        assertion_method_relation: collections
            .string_set(slot::ASSERTION_METHOD_RELATION, "assertionMethodRelation")?,
        key_agreement_relation: collections.string_set(slot::KEY_AGREEMENT_RELATION, "keyAgreementRelation")?,
        capability_invocation_relation: collections
            .string_set(slot::CAPABILITY_INVOCATION_RELATION, "capabilityInvocationRelation")?,
        capability_delegation_relation: collections
            .string_set(slot::CAPABILITY_DELEGATION_RELATION, "capabilityDelegationRelation")?,
        services: collections.services()?,
    })
}

/// Convenience composition: bytes → charged state → snapshot.
pub fn snapshot_from_bytes(bytes: &[u8]) -> Result<DidLedgerSnapshot, BackendError> {
    decode_ledger_snapshot(&charged_state_from_bytes(bytes)?)
}

// ─────────────────────────────────────────────────────────────────────
// Collection walking
// ─────────────────────────────────────────────────────────────────────

/// Borrows of the two outer-array chunks (`[0]` = fields 0–3,
/// `[1]` = fields 4–18).
struct StateChunks<'a> {
    chunk0: &'a compact_runtime::Array<StateValue<DefaultDB>, DefaultDB>,
    chunk1: &'a compact_runtime::Array<StateValue<DefaultDB>, DefaultDB>,
}

impl<'a> StateChunks<'a> {
    fn from_state(state: &'a ChargedState<DefaultDB>) -> Result<Self, BackendError> {
        let outer = match state.get_ref() {
            StateValue::Array(outer) => outer,
            other => {
                return Err(BackendError::Decode(format!(
                    "expected outer StateValue::Array, found {}",
                    variant_name(other)
                )));
            }
        };
        let chunk = |index: usize| -> Result<_, BackendError> {
            match outer.get(index) {
                Some(StateValue::Array(inner)) => Ok(inner),
                Some(other) => Err(BackendError::Decode(format!(
                    "expected inner StateValue::Array at [{index}], found {} (flat/legacy layout?)",
                    variant_name(other)
                ))),
                None => Err(BackendError::Decode(format!(
                    "outer state array has no [{index}] chunk"
                ))),
            }
        };
        Ok(Self {
            chunk0: chunk(0)?,
            chunk1: chunk(1)?,
        })
    }

    /// Raw byte payload of a `Cell` at `[chunk][slot]`.
    fn raw_bytes_cell(&self, chunk: usize, slot: usize, field: &str) -> Result<Vec<u8>, BackendError> {
        let arr = if chunk == 0 { self.chunk0 } else { self.chunk1 };
        let sv = arr
            .get(slot)
            .ok_or_else(|| BackendError::Decode(format!("missing ledger slot [{chunk}][{slot}] ({field})")))?;
        let av = cell_value(sv, field)?;
        aligned_bytes(&av)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| BackendError::Decode(format!("field {field}: cell has no byte payload")))
    }
}

/// Borrow of the `[1]` chunk (fields 4–18) of the outer state array.
struct CollectionSlots<'a> {
    inner: &'a compact_runtime::Array<StateValue<DefaultDB>, DefaultDB>,
}

impl CollectionSlots<'_> {
    fn slot(&self, index: usize, field: &str) -> Result<&StateValue<DefaultDB>, BackendError> {
        self.inner
            .get(index)
            .ok_or_else(|| BackendError::Decode(format!("missing ledger slot [1][{index}] ({field})")))
    }

    /// Decode a `Set<Opaque<"string">>` field: elements are the map KEYS
    /// (the values are placeholder cells). Returned sorted for
    /// determinism — the on-chain map has no stable iteration order.
    fn string_set(&self, index: usize, field: &str) -> Result<Vec<String>, BackendError> {
        let mut out = match self.slot(index, field)? {
            // TS-scaffolded empty slots may still be Null when never written.
            StateValue::Null => Vec::new(),
            StateValue::Map(map) => {
                let mut v = Vec::new();
                for entry in map.iter() {
                    v.push(key_string(&entry.0, field)?);
                }
                v
            }
            other => {
                return Err(BackendError::Decode(format!(
                    "field {field}: expected Map/Null, found {}",
                    variant_name(other)
                )));
            }
        };
        out.sort();
        Ok(out)
    }

    /// Decode a `Map<Opaque<"string">, T>` field into `(key, T)` pairs.
    ///
    /// Map-value cells are **alignment-encoded**: one atom per leaf field
    /// (strings as raw UTF-8 bytes, enums as one byte, Jubjub points as
    /// two 32-byte coordinates). NOT `decode_via_field_repr` — that
    /// expects the in-circuit field-element packing, which has a
    /// different arity than the cell's atom sequence.
    fn typed_map<F, O>(&self, index: usize, field: &str, convert: F) -> Result<BTreeMap<String, O>, BackendError>
    where
        F: Fn(&mut AtomCursor<'_>) -> Result<O, BackendError>,
    {
        match self.slot(index, field)? {
            StateValue::Null => Ok(BTreeMap::new()),
            StateValue::Map(map) => {
                let mut out = BTreeMap::new();
                for entry in map.iter() {
                    let key = key_string(&entry.0, field)?;
                    let av = cell_value(&entry.1, field)?;
                    let mut cursor = AtomCursor::new(&av, field);
                    let value = convert(&mut cursor)?;
                    cursor.finish()?;
                    out.insert(key, value);
                }
                Ok(out)
            }
            other => Err(BackendError::Decode(format!(
                "field {field}: expected Map/Null, found {}",
                variant_name(other)
            ))),
        }
    }

    fn verification_methods(&self) -> Result<BTreeMap<String, LedgerVerificationMethod>, BackendError> {
        self.typed_map(slot::VERIFICATION_METHODS, "verificationMethods", |c| {
            Ok(LedgerVerificationMethod {
                id: c.take_string("id")?,
                typ: verification_method_type(c.take_u8("typ")?)?,
                public_key_jwk: LedgerPublicKeyJwk {
                    kty: key_type(c.take_u8("kty")?)?,
                    crv: curve_type(c.take_u8("crv")?)?,
                    x: c.take_string("x")?,
                    y: c.take_string("y")?,
                },
            })
        })
    }

    fn schnorr_jubjub_verification_methods(
        &self,
    ) -> Result<BTreeMap<String, LedgerSchnorrJubjubVerificationMethod>, BackendError> {
        self.typed_map(
            slot::SCHNORR_JUBJUB_VERIFICATION_METHODS,
            "schnorrJubjubVerificationMethods",
            |c| {
                let id = c.take_string("id")?;
                let x = c.take_coordinate_hex("publicKey.x")?;
                let y = c.take_coordinate_hex("publicKey.y")?;
                Ok(LedgerSchnorrJubjubVerificationMethod {
                    id,
                    public_key: JubjubPointHex::new(NewJubjubPointHex { x, y })
                        .map_err(|e| BackendError::Decode(format!("publicKey: {e}")))?,
                })
            },
        )
    }

    fn services(&self) -> Result<BTreeMap<String, LedgerService>, BackendError> {
        self.typed_map(slot::SERVICES, "services", |c| {
            Ok(LedgerService {
                id: c.take_string("id")?,
                typ: c.take_string("typ")?,
                service_endpoint: c.take_string("serviceEndpoint")?,
            })
        })
    }
}

/// Sequential reader over an alignment-encoded cell's atoms.
struct AtomCursor<'a> {
    av: &'a AlignedValue,
    pos: usize,
    field: &'a str,
}

impl<'a> AtomCursor<'a> {
    fn new(av: &'a AlignedValue, field: &'a str) -> Self {
        Self { av, pos: 0, field }
    }

    fn take_bytes(&mut self, leaf: &str) -> Result<&'a [u8], BackendError> {
        let atom =
            self.av.value.0.get(self.pos).ok_or_else(|| {
                BackendError::Decode(format!("field {}: missing atom {} ({leaf})", self.field, self.pos))
            })?;
        self.pos += 1;
        Ok(atom.0.as_slice())
    }

    fn take_string(&mut self, leaf: &str) -> Result<String, BackendError> {
        let field = self.field;
        let bytes = self.take_bytes(leaf)?;
        String::from_utf8(bytes.to_vec())
            .map_err(|e| BackendError::Decode(format!("field {field}.{leaf}: not UTF-8: {e}")))
    }

    fn take_u8(&mut self, leaf: &str) -> Result<u8, BackendError> {
        let field = self.field;
        let bytes = self.take_bytes(leaf)?;
        match bytes {
            [] => Ok(0), // zero-valued atoms may be stored empty
            [b] => Ok(*b),
            _ => Err(BackendError::Decode(format!(
                "field {field}.{leaf}: expected 1-byte atom, got {} bytes",
                bytes.len()
            ))),
        }
    }

    /// A Jubjub coordinate atom: ≤32 little-endian bytes, hex-padded to
    /// exactly 32 (the `JubjubPointHex` invariant).
    fn take_coordinate_hex(&mut self, leaf: &str) -> Result<String, BackendError> {
        let field = self.field;
        let bytes = self.take_bytes(leaf)?;
        if bytes.len() > 32 {
            return Err(BackendError::Decode(format!(
                "field {field}.{leaf}: coordinate atom is {} bytes (>32)",
                bytes.len()
            )));
        }
        let mut out = bytes.to_vec();
        out.resize(32, 0);
        Ok(hex::encode(out))
    }

    fn finish(self) -> Result<(), BackendError> {
        if self.pos == self.av.value.0.len() {
            Ok(())
        } else {
            Err(BackendError::Decode(format!(
                "field {}: {} trailing atom(s) after decode",
                self.field,
                self.av.value.0.len() - self.pos
            )))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Leaf conversions
// ─────────────────────────────────────────────────────────────────────

fn decode_err(field: &str, e: &dyn std::fmt::Debug) -> BackendError {
    BackendError::Decode(format!("ledger field {field}: {e:?}"))
}

fn variant_name(sv: &StateValue<DefaultDB>) -> &'static str {
    match sv {
        StateValue::Null => "Null",
        StateValue::Cell(_) => "Cell",
        StateValue::Map(_) => "Map",
        StateValue::Array(_) => "Array",
        _ => "other",
    }
}

/// Map keys are `Opaque<"string">` cells: raw UTF-8 bytes.
fn key_string(key: &AlignedValue, field: &str) -> Result<String, BackendError> {
    let bytes =
        aligned_bytes(key).ok_or_else(|| BackendError::Decode(format!("field {field}: key has no byte payload")))?;
    String::from_utf8(bytes.to_vec()).map_err(|e| BackendError::Decode(format!("field {field}: key is not UTF-8: {e}")))
}

fn cell_value(value: &StateValue<DefaultDB>, field: &str) -> Result<AlignedValue, BackendError> {
    match value {
        StateValue::Cell(av) => Ok((**av).clone()),
        other => Err(BackendError::Decode(format!(
            "field {field}: expected Cell value, found {}",
            variant_name(other)
        ))),
    }
}

/// The snapshot's single-string controller-key field: x coordinate only
/// (see module docs / issue #3).
fn jubjub_x_hex(point: &JubjubPoint) -> Result<String, BackendError> {
    coordinate_hex(point.x(), "x")
}

fn coordinate_hex(coordinate: Option<compact_runtime::Fr>, which: &str) -> Result<String, BackendError> {
    let fr = coordinate
        .ok_or_else(|| BackendError::Decode(format!("JubjubPoint {which}: identity point has no coordinates")))?;
    let mut bytes = fr.as_le_bytes();
    // Fr reprs are 32 bytes; pad defensively so JubjubPointHex's
    // exact-length validation can't be tripped by a short repr.
    bytes.resize(32, 0);
    Ok(hex::encode(bytes))
}

// Enum discriminants follow the generated `#[repr]`-style numbering in
// contract/generated.rs (`VerificationMethodType`, `KeyType`, `CurveType`).

fn verification_method_type(d: u8) -> Result<VerificationMethodType, BackendError> {
    match d {
        0 => Ok(VerificationMethodType::Undefined),
        1 => Ok(VerificationMethodType::JsonWebKey),
        other => Err(BackendError::Decode(format!(
            "unknown VerificationMethodType discriminant {other}"
        ))),
    }
}

fn key_type(d: u8) -> Result<KeyType, BackendError> {
    match d {
        0 => Ok(KeyType::EC),
        1 => Ok(KeyType::RSA),
        2 => Ok(KeyType::oct),
        3 => Ok(KeyType::OKP),
        other => Err(BackendError::Decode(format!("unknown KeyType discriminant {other}"))),
    }
}

fn curve_type(d: u8) -> Result<CurveType, BackendError> {
    match d {
        0 => Ok(CurveType::Ed25519),
        1 => Ok(CurveType::X25519),
        2 => Ok(CurveType::Jubjub),
        3 => Ok(CurveType::P256),
        4 => Ok(CurveType::Secp256k1),
        5 => Ok(CurveType::BLS12381G1),
        6 => Ok(CurveType::BLS12381G2),
        other => Err(BackendError::Decode(format!("unknown CurveType discriminant {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use compact_runtime::{Alignment, AlignmentAtom, Map, Value, ValueAtom, new_array, new_cell, new_map};

    use super::*;
    use crate::contract as generated;

    /// Extract the `AlignedValue` out of a `StateValue::Cell`.
    fn cell_av(sv: &StateValue<DefaultDB>) -> AlignedValue {
        match sv {
            StateValue::Cell(av) => (**av).clone(),
            other => panic!("expected Cell, got {}", variant_name(other)),
        }
    }

    /// Replace the atom at `index` of a cell's aligned value. The decoder
    /// never re-validates alignment, so tests can inject arbitrary bytes
    /// (unknown discriminants, oversized coordinates, multi-byte "u8"s).
    fn with_mutated_atom(sv: &StateValue<DefaultDB>, index: usize, bytes: Vec<u8>) -> StateValue<DefaultDB> {
        let mut av = cell_av(sv);
        av.value.0[index] = ValueAtom(bytes);
        new_cell::<DefaultDB, _>(av)
    }

    /// Build a chain-shaped (nested `[4][15]`) DID contract state the way
    /// the TS reference constructor lays it out. NOT via the generated
    /// `initial_state` — that scaffold is flat (compact A29 bug) and
    /// deliberately unrepresentative of on-chain state.
    fn chain_shaped_state() -> ChargedState<DefaultDB> {
        chain_state_with(|_, _| {})
    }

    /// Like [`chain_shaped_state`], but lets a test corrupt individual
    /// slots of chunk `[0]` / chunk `[1]` before the outer array is built.
    fn sample_vm_cell() -> StateValue<DefaultDB> {
        new_cell(generated::VerificationMethod {
            id: "#key-1".to_string().into(),
            typ: generated::VerificationMethodType::JsonWebKey,
            publicKeyJwk: generated::PublicKeyJwk {
                kty: generated::KeyType::OKP,
                crv: generated::CurveType::Ed25519,
                x: "b64url-x".to_string().into(),
                y: "".to_string().into(),
            },
        })
    }

    fn sample_sjvm_cell() -> StateValue<DefaultDB> {
        new_cell(generated::SchnorrJubjubVerificationMethod {
            id: "#key-jub".to_string().into(),
            publicKey: JubjubPoint::generator(),
        })
    }

    fn sample_service_cell() -> StateValue<DefaultDB> {
        new_cell(generated::Service {
            id: "#svc-1".to_string().into(),
            typ: "LinkedDomains".to_string().into(),
            serviceEndpoint: "https://example.com".to_string().into(),
        })
    }

    fn opaque_key(s: &str) -> AlignedValue {
        let sv = new_cell::<DefaultDB, _>(compact_runtime::std_lib::OpaqueString(s.to_string()));
        cell_av(&sv)
    }

    fn map_of(entries: Vec<(&str, StateValue<DefaultDB>)>) -> StateValue<DefaultDB> {
        raw_map_of(entries.into_iter().map(|(k, v)| (opaque_key(k), v)).collect())
    }

    fn raw_map_of(entries: Vec<(AlignedValue, StateValue<DefaultDB>)>) -> StateValue<DefaultDB> {
        let mut m: Map<AlignedValue, StateValue<DefaultDB>, DefaultDB> = Map::new();
        for (k, v) in entries {
            m = m.insert(k, v);
        }
        StateValue::Map(m)
    }

    /// Sets store the element as key with a placeholder cell value.
    fn set_of(elems: Vec<&str>) -> StateValue<DefaultDB> {
        map_of(elems.into_iter().map(|e| (e, new_cell(0u8))).collect())
    }

    fn chain_state_with(
        mutate: impl FnOnce(&mut Vec<StateValue<DefaultDB>>, &mut Vec<StateValue<DefaultDB>>),
    ) -> ChargedState<DefaultDB> {
        let point = JubjubPoint::generator();
        let vm = sample_vm_cell();
        let sjvm = sample_sjvm_cell();
        let service = sample_service_cell();

        // chunk [0]: fields 0..4
        let mut chunk0 = vec![
            new_cell(2u32),                                            // contractVersion
            new_cell(point),                                           // controllerPublicKey
            new_cell(point),                                           // recoveryAuthorityPublicKey
            new_cell(generated::ContractAddress { bytes: [7u8; 32] }), // id
        ];
        // chunk [1]: fields 4..19
        let mut chunk1 = vec![
            set_of(vec!["did:example:alias1", "did:example:alias0"]), // alsoKnownAs
            new_cell(3u64),                                           // version
            new_cell(1_700_000_000_000u64),                           // created
            new_cell(1_700_000_100_000u64),                           // updated
            new_cell(false),                                          // deactivated
            new_cell(true),                                           // active
            new_cell(5u64),                                           // operationCount
            map_of(vec![("#key-1", vm)]),                             // verificationMethods
            map_of(vec![("#key-jub", sjvm)]),                         // schnorrJubjubVerificationMethods
            set_of(vec!["#key-1"]),                                   // authenticationRelation
            new_map(),                                                // assertionMethodRelation (empty Map)
            StateValue::Null,                                         // keyAgreementRelation (never-written Null)
            new_map(),                                                // capabilityInvocationRelation
            new_map(),                                                // capabilityDelegationRelation
            map_of(vec![("#svc-1", service)]),                        // services
        ];
        mutate(&mut chunk0, &mut chunk1);
        ChargedState::new(new_array::<DefaultDB>(vec![new_array(chunk0), new_array(chunk1)]))
    }

    #[test]
    fn decodes_chain_shaped_state_into_snapshot() {
        let state = chain_shaped_state();
        let snap = decode_ledger_snapshot(&state).expect("decode succeeds");

        assert_eq!(snap.contract_version, 2);
        assert_eq!(snap.id_hex, hex::encode([7u8; 32]));
        assert!(snap.active);
        assert!(!snap.deactivated);
        assert_eq!(snap.version, 3);
        assert_eq!(snap.operation_count, 5);
        assert_eq!(snap.created_ms, 1_700_000_000_000);
        assert_eq!(snap.updated_ms, 1_700_000_100_000);

        // controller key: x coordinate of the generator, LE hex, 64 chars.
        let gen_x = JubjubPoint::generator().x().expect("generator has coordinates");
        let mut gen_x_bytes = gen_x.as_le_bytes();
        gen_x_bytes.resize(32, 0);
        assert_eq!(snap.controller_public_key_hex, hex::encode(gen_x_bytes));

        // sets: sorted, Null and empty-Map both mean empty.
        assert_eq!(
            snap.also_known_as,
            vec!["did:example:alias0".to_string(), "did:example:alias1".to_string()]
        );
        assert_eq!(snap.authentication_relation, vec!["#key-1".to_string()]);
        assert!(snap.assertion_method_relation.is_empty());
        assert!(snap.key_agreement_relation.is_empty());

        // typed maps.
        let vm = snap.verification_methods.get("#key-1").expect("vm present");
        assert_eq!(vm.id, "#key-1");
        assert_eq!(vm.typ, VerificationMethodType::JsonWebKey);
        assert_eq!(vm.public_key_jwk.kty, KeyType::OKP);
        assert_eq!(vm.public_key_jwk.crv, CurveType::Ed25519);
        assert_eq!(vm.public_key_jwk.x, "b64url-x");
        assert_eq!(vm.public_key_jwk.y, "");

        let sj = snap
            .schnorr_jubjub_verification_methods
            .get("#key-jub")
            .expect("schnorr vm present");
        assert_eq!(sj.id, "#key-jub");
        assert_eq!(sj.public_key.x(), snap.controller_public_key_hex);

        let svc = snap.services.get("#svc-1").expect("service present");
        assert_eq!(svc.typ, "LinkedDomains");
        assert_eq!(svc.service_endpoint, "https://example.com");
    }

    #[test]
    fn flat_state_is_rejected_with_layout_error() {
        // The A29 flat scaffold shape: outer array whose [1] is a Cell,
        // not the [1] chunk. Must fail loudly, not mis-decode.
        let flat = ChargedState::new(new_array::<DefaultDB>(vec![
            new_cell(2u32),
            new_cell(JubjubPoint::generator()),
        ]));
        let err = decode_ledger_snapshot(&flat).expect_err("flat layout must fail");
        assert!(matches!(err, BackendError::Decode(_)), "got {err:?}");
    }

    #[test]
    fn snapshot_round_trips_through_contract_state_bytes() {
        // Serialize a full ContractState around the charged state, then
        // charged_state_from_bytes -> decode: the exact indexer path.
        let state = chain_shaped_state();
        let contract_state =
            ContractState::<DefaultDB>::new(state.get_ref().clone(), Default::default(), Default::default());
        let mut bytes = Vec::new();
        midnight_serialize::tagged_serialize(&contract_state, &mut bytes).expect("serialize");

        let snap_direct = decode_ledger_snapshot(&state).expect("direct decode");
        let snap_via_bytes = snapshot_from_bytes(&bytes).expect("bytes decode");
        assert_eq!(snap_direct, snap_via_bytes);
    }

    // ── error paths ──────────────────────────────────────────────────

    /// Assert `result` is a `Decode` error whose message contains `needle`.
    fn assert_decode_err<T: std::fmt::Debug>(result: Result<T, BackendError>, needle: &str) {
        match result {
            Err(BackendError::Decode(msg)) => {
                assert!(msg.contains(needle), "message {msg:?} does not contain {needle:?}");
            }
            other => panic!("expected Decode error containing {needle:?}, got {other:?}"),
        }
    }

    #[test]
    fn snapshot_from_bytes_rejects_garbage_bytes() {
        assert_decode_err(snapshot_from_bytes(&[0xde, 0xad, 0xbe, 0xef]), "deserialize failed");
        assert_decode_err(snapshot_from_bytes(&[]), "deserialize failed");
    }

    #[test]
    fn non_array_outer_state_is_rejected() {
        let state = ChargedState::<DefaultDB>::new(StateValue::Null);
        assert_decode_err(decode_ledger_snapshot(&state), "expected outer StateValue::Array");
    }

    #[test]
    fn missing_second_chunk_is_rejected() {
        let state = ChargedState::new(new_array::<DefaultDB>(vec![new_array(vec![])]));
        assert_decode_err(decode_ledger_snapshot(&state), "no [1] chunk");
    }

    #[test]
    fn missing_ledger_slot_is_rejected() {
        // Drop the id cell ([0][3]) — the first raw read in decode order.
        let state = chain_state_with(|chunk0, _| {
            chunk0.truncate(3);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "missing ledger slot [0][3] (id)");
    }

    #[test]
    fn non_map_collection_slot_is_rejected() {
        // alsoKnownAs must be Map/Null; a scalar Cell is a layout error.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::ALSO_KNOWN_AS] = new_cell(1u8);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "field alsoKnownAs: expected Map/Null, found Cell",
        );
    }

    #[test]
    fn non_map_typed_slot_is_rejected() {
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::SERVICES] = new_array(vec![]);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "field services: expected Map/Null, found Array",
        );
    }

    #[test]
    fn non_utf8_set_key_is_rejected() {
        let bad_key = AlignedValue::new(
            Value(vec![ValueAtom(vec![0xff, 0xfe])]),
            Alignment::singleton(AlignmentAtom::Bytes { length: 2 }),
        )
        .expect("2-byte atom fits Bytes{2}");
        let state = chain_state_with(move |_, chunk1| {
            chunk1[slot::ALSO_KNOWN_AS] = raw_map_of(vec![(bad_key, new_cell(0u8))]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "key is not UTF-8");
    }

    #[test]
    fn non_utf8_typed_map_key_is_rejected() {
        let bad_key = AlignedValue::new(
            Value(vec![ValueAtom(vec![0x80])]),
            Alignment::singleton(AlignmentAtom::Bytes { length: 1 }),
        )
        .expect("1-byte atom fits Bytes{1}");
        let state = chain_state_with(move |_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] = raw_map_of(vec![(bad_key, sample_vm_cell())]);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "field verificationMethods: key is not UTF-8",
        );
    }

    #[test]
    fn non_cell_map_value_is_rejected() {
        // A collection entry whose value is an Array, not a Cell.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] = map_of(vec![("#key-1", new_array(vec![]))]);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "field verificationMethods: expected Cell value, found Array",
        );
    }

    #[test]
    fn unknown_verification_method_type_discriminant_is_rejected() {
        // VM cell atoms: [id, typ, kty, crv, x, y] — set typ to 9.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] =
                map_of(vec![("#key-1", with_mutated_atom(&sample_vm_cell(), 1, vec![9]))]);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "unknown VerificationMethodType discriminant 9",
        );
    }

    #[test]
    fn unknown_key_type_discriminant_is_rejected() {
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] =
                map_of(vec![("#key-1", with_mutated_atom(&sample_vm_cell(), 2, vec![9]))]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "unknown KeyType discriminant 9");
    }

    #[test]
    fn unknown_curve_type_discriminant_is_rejected() {
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] =
                map_of(vec![("#key-1", with_mutated_atom(&sample_vm_cell(), 3, vec![9]))]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "unknown CurveType discriminant 9");
    }

    #[test]
    fn enum_discriminant_tables_cover_all_variants() {
        // Direct checks of every discriminant the on-chain cells can carry.
        assert_eq!(verification_method_type(0).unwrap(), VerificationMethodType::Undefined);
        assert_eq!(verification_method_type(1).unwrap(), VerificationMethodType::JsonWebKey);
        assert_eq!(key_type(0).unwrap(), KeyType::EC);
        assert_eq!(key_type(1).unwrap(), KeyType::RSA);
        assert_eq!(key_type(2).unwrap(), KeyType::oct);
        assert_eq!(key_type(3).unwrap(), KeyType::OKP);
        assert_eq!(curve_type(0).unwrap(), CurveType::Ed25519);
        assert_eq!(curve_type(1).unwrap(), CurveType::X25519);
        assert_eq!(curve_type(2).unwrap(), CurveType::Jubjub);
        assert_eq!(curve_type(3).unwrap(), CurveType::P256);
        assert_eq!(curve_type(4).unwrap(), CurveType::Secp256k1);
        assert_eq!(curve_type(5).unwrap(), CurveType::BLS12381G1);
        assert_eq!(curve_type(6).unwrap(), CurveType::BLS12381G2);
        assert!(verification_method_type(2).is_err());
        assert!(key_type(4).is_err());
        assert!(curve_type(7).is_err());
    }

    #[test]
    fn multi_byte_enum_atom_is_rejected() {
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] =
                map_of(vec![("#key-1", with_mutated_atom(&sample_vm_cell(), 1, vec![1, 1]))]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "expected 1-byte atom, got 2 bytes");
    }

    #[test]
    fn empty_enum_atom_decodes_as_zero() {
        // Zero-valued atoms may be stored with their bytes stripped.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::VERIFICATION_METHODS] =
                map_of(vec![("#key-1", with_mutated_atom(&sample_vm_cell(), 1, vec![]))]);
        });
        let snap = decode_ledger_snapshot(&state).expect("empty atom is zero");
        assert_eq!(
            snap.verification_methods.get("#key-1").unwrap().typ,
            VerificationMethodType::Undefined
        );
    }

    #[test]
    fn oversized_coordinate_atom_is_rejected() {
        // Schnorr VM cell atoms: [id, publicKey.x, publicKey.y].
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::SCHNORR_JUBJUB_VERIFICATION_METHODS] = map_of(vec![(
                "#key-jub",
                with_mutated_atom(&sample_sjvm_cell(), 1, vec![0xaa; 33]),
            )]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "coordinate atom is 33 bytes (>32)");
    }

    #[test]
    fn short_coordinate_atom_is_zero_padded() {
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::SCHNORR_JUBJUB_VERIFICATION_METHODS] = map_of(vec![(
                "#key-jub",
                with_mutated_atom(&sample_sjvm_cell(), 1, vec![0x01]),
            )]);
        });
        let snap = decode_ledger_snapshot(&state).expect("short coordinate pads");
        let sj = snap.schnorr_jubjub_verification_methods.get("#key-jub").unwrap();
        assert_eq!(sj.public_key.x(), format!("01{}", "00".repeat(31)));
    }

    #[test]
    fn trailing_atoms_are_rejected() {
        // A 6-atom VM cell in the 3-atom services shape: the first three
        // atoms decode as strings, then finish() must flag the leftovers.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::SERVICES] = map_of(vec![("#svc-1", sample_vm_cell())]);
        });
        assert_decode_err(
            decode_ledger_snapshot(&state),
            "field services: 3 trailing atom(s) after decode",
        );
    }

    #[test]
    fn missing_atom_is_rejected() {
        // A 1-atom cell in the 3-atom services shape.
        let state = chain_state_with(|_, chunk1| {
            chunk1[slot::SERVICES] = map_of(vec![("#svc-1", new_cell(0u8))]);
        });
        assert_decode_err(decode_ledger_snapshot(&state), "missing atom 1 (typ)");
    }
}
