// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Pure wallet-local coin-store transformations from the upstream witnesses.

use std::collections::BTreeMap;

/// Stored qualified coin value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredCoin {
    /// Coin nonce.
    pub nonce: [u8; 32],
    /// Coin color.
    pub color: [u8; 32],
    /// Coin value.
    pub value: u128,
    /// Merkle-tree index for the qualified coin.
    pub mt_index: u128,
}

/// Private state needed by account witnesses, represented without a runtime dependency.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoinStorePrivateState {
    /// X25519 account encryption secret, when known.
    pub enc_secret_key: Option<[u8; 32]>,
    /// One held coin per color, keyed by lowercase hex color.
    pub coins: BTreeMap<String, StoredCoin>,
}

/// Construct an empty coin store, optionally carrying the viewing capability.
pub fn empty_coin_store(enc_secret_key: Option<[u8; 32]>) -> CoinStorePrivateState {
    CoinStorePrivateState {
        enc_secret_key,
        coins: BTreeMap::new(),
    }
}

/// Return a new state with `coin` inserted/replaced under its color.
pub fn with_coin(state: &CoinStorePrivateState, coin: StoredCoin) -> CoinStorePrivateState {
    let mut next = state.clone();
    next.coins.insert(hex::encode(coin.color), coin);
    next
}

/// Return a new state with the coin for `color` removed.
pub fn without_coin(state: &CoinStorePrivateState, color: &[u8; 32]) -> CoinStorePrivateState {
    let mut next = state.clone();
    next.coins.remove(&hex::encode(color));
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_insert_remove_are_pure() {
        let state = empty_coin_store(Some([9; 32]));
        assert!(state.coins.is_empty());
        let coin = StoredCoin {
            nonce: [1; 32],
            color: [2; 32],
            value: 5,
            mt_index: 7,
        };
        let inserted = with_coin(&state, coin.clone());
        assert!(state.coins.is_empty());
        assert_eq!(inserted.coins.get(&hex::encode([2; 32])), Some(&coin));
        let removed = without_coin(&inserted, &[2; 32]);
        assert!(removed.coins.is_empty());
        assert_eq!(removed.enc_secret_key, Some([9; 32]));
    }
}
