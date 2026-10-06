// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0

//! Explicit conversions between method-layer and Compact runtime identifiers.
//!
//! Keeping these conversions in the runtime leaf prevents DID parsing and
//! resolution consumers from compiling the Ledger, Compact, and proof-system
//! dependency graph.

use midnight_base_crypto::hash::HashOutput;
use midnight_did_method::midnight_did::ContractAddress as MethodContractAddress;

/// Convert a runtime-independent method address into the Compact runtime type.
pub fn to_runtime_contract_address(address: MethodContractAddress) -> midnight_compact_runtime::ContractAddress {
    midnight_compact_runtime::ContractAddress(HashOutput(address.0))
}

/// Convert a Compact runtime address into the runtime-independent method type.
pub fn from_runtime_contract_address(address: midnight_compact_runtime::ContractAddress) -> MethodContractAddress {
    MethodContractAddress(address.0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_address_round_trips_across_runtime_boundary() {
        let method = MethodContractAddress([0x5a; 32]);
        assert_eq!(
            from_runtime_contract_address(to_runtime_contract_address(method)),
            method
        );
    }
}
