// This file is part of MediaNoxLabs/midnight-identity.
// Copyright (C) 2026 Midnight Foundation
// SPDX-License-Identifier: Apache-2.0
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Runtime-neutral inbox discovery over an abstract read-only ledger view.

use crate::inbox::{PlainCoin, open_inbox_entry};

/// Read-only view of the account ledger inbox needed for discovery.
pub trait LedgerInboxView {
    /// Number of candidate inbox slots to scan, newest last.
    fn inbox_count(&self) -> u64;
    /// Return the fixed 192-byte entry at `index`, or `None` when absent.
    fn inbox_entry(&self, index: u64) -> Option<&[u8]>;
    /// Return whether the decrypted coin at `index` matches the chain coin
    /// commitment/qualified-coin evidence associated with the depositing call.
    ///
    /// AEAD only proves that the entry was encrypted to the account public key;
    /// the contract cannot prove that the plaintext matches the coin supplied
    /// to `deposit_shielded`. Implementations must recompute/check the
    /// chain-visible coin evidence here before discovery returns the coin.
    fn inbox_entry_matches_chain_coin(&self, index: u64, coin: &PlainCoin) -> bool;
}

/// Recovered coin plus its inbox ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredCoin {
    /// Decrypted coin.
    pub coin: PlainCoin,
    /// Ordinal of the inbox entry the coin was recovered from.
    pub inbox_index: u64,
}

/// Walk the inbox with the account viewing capability, skipping unsupported or
/// unauthentic entries instead of aborting discovery.
pub fn inbox_walk(view: &impl LedgerInboxView, enc_secret_key: &[u8; 32]) -> Vec<DiscoveredCoin> {
    let mut out = Vec::new();
    for i in 0..view.inbox_count() {
        let Some(entry) = view.inbox_entry(i) else {
            continue;
        };
        if let Ok(coin) = open_inbox_entry(enc_secret_key, entry)
            && view.inbox_entry_matches_chain_coin(i, &coin)
        {
            out.push(DiscoveredCoin { coin, inbox_index: i });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::inbox::{generate_enc_key_pair, seal_inbox_entry};

    struct View {
        entries: Vec<Option<Vec<u8>>>,
        coins: Vec<Option<PlainCoin>>,
    }
    impl LedgerInboxView for View {
        fn inbox_count(&self) -> u64 {
            self.entries.len() as u64
        }
        fn inbox_entry(&self, index: u64) -> Option<&[u8]> {
            self.entries[index as usize].as_deref()
        }
        fn inbox_entry_matches_chain_coin(&self, index: u64, coin: &PlainCoin) -> bool {
            self.coins[index as usize].as_ref() == Some(coin)
        }
    }

    #[test]
    fn walk_skips_absent_and_bad_entries() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let keys = generate_enc_key_pair(&mut rng);
        let coin = PlainCoin {
            nonce: [1; 32],
            color: [2; 32],
            value: 3,
        };
        let entry = seal_inbox_entry(&mut rng, &keys.public_key, &coin).unwrap();
        let mut forged = coin.clone();
        forged.value += 1;
        let forged_entry = seal_inbox_entry(&mut rng, &keys.public_key, &forged).unwrap();
        let view = View {
            entries: vec![
                None,
                Some(vec![0; 192]),
                Some(entry.to_vec()),
                Some(forged_entry.to_vec()),
            ],
            coins: vec![None, None, Some(coin.clone()), Some(coin.clone())],
        };
        assert_eq!(
            inbox_walk(&view, &keys.secret_key),
            vec![DiscoveredCoin { coin, inbox_index: 2 }]
        );
    }
}
