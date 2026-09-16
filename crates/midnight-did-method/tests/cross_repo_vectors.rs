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

//! Self-contained golden vectors imported from read-only consumer snapshots:
//! - MediaNoxLabs/oxid @ c9b9bc677836f19a5cd28306d8584a7d8623fd57,
//!   `crates/identity/domain/src/lib.rs` (Apache-2.0, SPDX header).
//! - input-output-hk/lace-id-portal @ ad54dd8de70a10a070e649633002639d263fa4c0,
//!   `crates/did-midnight/src/lib.rs` (repository had no LICENSE file in the
//!   recorded checkout; source path has no SPDX header).

use midnight_did_method::holder_binding::hash_offchain_did_method_id;
use midnight_did_method::{
    MAX_MIDNIGHT_DID_CHARACTERS, MidnightDidError, MidnightNetwork, parse_midnight_did_parts,
    parse_midnight_did_string, parse_midnight_key_id,
};

const ADDR: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const UPPER_ADDR: &str = "ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
const KEY_ID: &str = "did:midnight:testnet:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef#key-assert";

#[test]
fn portal_four_segment_parser_vectors() {
    let parsed = parse_midnight_did_parts(&format!("did:midnight:testnet:{ADDR}")).unwrap();
    assert_eq!(parsed.network, MidnightNetwork::Testnet);
    assert_eq!(parsed.identifier, ADDR);

    let parsed = parse_midnight_did_parts(&format!("did:midnight:testnet:{UPPER_ADDR}")).unwrap();
    assert_eq!(parsed.identifier, UPPER_ADDR.to_ascii_lowercase());

    assert!(matches!(
        parse_midnight_did_parts(&format!("https:midnight:testnet:{ADDR}")),
        Err(MidnightDidError::BadFormat)
    ));
    assert!(matches!(
        parse_midnight_did_parts(&format!("did:other:testnet:{ADDR}")),
        Err(MidnightDidError::BadFormat)
    ));
    assert!(matches!(
        parse_midnight_did_parts("did:midnight:testnet"),
        Err(MidnightDidError::BadFormat)
    ));
    assert!(matches!(
        parse_midnight_did_parts(&format!("did:midnight:testnet:{ADDR}:extra")),
        Err(MidnightDidError::BadFormat)
    ));
    assert!(matches!(
        parse_midnight_did_parts(&format!("did:midnight::{ADDR}")),
        Err(MidnightDidError::UnknownNetwork)
    ));
    assert!(matches!(
        parse_midnight_did_parts("did:midnight:testnet:abc"),
        Err(MidnightDidError::BadMethodSpecificId)
    ));
    assert!(matches!(
        parse_midnight_did_parts(&format!("did:midnight:testnet:{}", "g".repeat(64))),
        Err(MidnightDidError::BadMethodSpecificId)
    ));
}

#[test]
fn portal_key_id_uses_last_hash_and_retains_fragment() {
    let parsed = parse_midnight_key_id(KEY_ID).unwrap();
    assert_eq!(parsed.network, MidnightNetwork::Testnet);
    assert_eq!(parsed.identifier, ADDR);
    assert_eq!(parsed.did.0, format!("did:midnight:testnet:{ADDR}"));
    assert_eq!(parsed.fragment, "#key-assert");
    assert_eq!(parsed.key_id, KEY_ID);

    let with_hash_in_fragment = format!("did:midnight:testnet:{ADDR}#key#assert");
    assert!(matches!(
        parse_midnight_key_id(&with_hash_in_fragment),
        Err(MidnightDidError::BadMethodSpecificId)
    ));

    assert!(matches!(
        parse_midnight_key_id(&format!("did:midnight:testnet:{ADDR}")),
        Err(MidnightDidError::MissingFragment)
    ));
    assert!(matches!(
        parse_midnight_key_id(&format!("did:midnight:testnet:{ADDR}#")),
        Err(MidnightDidError::EmptyFragment)
    ));
    assert!(matches!(
        parse_midnight_key_id("did:midnight:testnet:short#key-assert"),
        Err(MidnightDidError::BadMethodSpecificId)
    ));
}

#[test]
fn oxid_bounded_and_character_hardening_vectors() {
    assert!(matches!(
        parse_midnight_did_string(&format!(" did:midnight:testnet:{ADDR}")),
        Err(MidnightDidError::InvalidCharacters)
    ));
    assert!(matches!(
        parse_midnight_did_string(&format!("did:midnight:testnet:{ADDR}\n")),
        Err(MidnightDidError::InvalidCharacters)
    ));
    assert!(matches!(
        parse_midnight_did_string(&format!("did:midnight:testnet:{ADDR}\u{7}")),
        Err(MidnightDidError::InvalidCharacters)
    ));
    assert!(matches!(
        parse_midnight_did_string(&"a".repeat(MAX_MIDNIGHT_DID_CHARACTERS + 1)),
        Err(MidnightDidError::TooLong)
    ));
}

#[test]
fn oxid_offchain_lowercase_and_base64url_vectors() {
    assert!(parse_midnight_did_string(&format!("did:midnight:offchain:{ADDR}:AQIDBA")).is_ok());
    assert!(matches!(
        parse_midnight_did_string(&format!("did:midnight:offchain:{UPPER_ADDR}")),
        Err(MidnightDidError::OffchainNotLowercase)
    ));
    for state in ["", "not+base64url", "abc=", "a"] {
        assert!(matches!(
            parse_midnight_did_string(&format!("did:midnight:offchain:{ADDR}:{state}")),
            Err(MidnightDidError::BadOffchainStateEncoding)
        ));
    }
}

#[test]
fn fragment_to_method_id_conventions_stay_distinct() {
    let padded_text_bytes = {
        let mut bytes = [0u8; 32];
        let fragment = b"#key-assert";
        bytes[..fragment.len()].copy_from_slice(fragment);
        bytes
    };
    let holder_binding_hash = hash_offchain_did_method_id("#key-assert");

    assert_eq!(
        hex::encode(padded_text_bytes),
        "236b65792d617373657274000000000000000000000000000000000000000000"
    );
    assert_eq!(
        hex::encode(holder_binding_hash),
        "1d1f9298d57001ce95a3e1eb1dfff2cbca9f7bfbbe4f0696367c548f8fc9938d"
    );
    assert_ne!(padded_text_bytes, holder_binding_hash);
}
