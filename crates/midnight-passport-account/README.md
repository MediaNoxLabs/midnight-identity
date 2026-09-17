<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-passport-account

Reusable, platform-neutral Rust primitives for the Midnight Passport
account-custody contract pinned by `midnight-passport-account-source`.

This crate ports the reusable signer/challenge, scoped-grant, InboxEntry v1,
coin-store, and inbox-discovery behavior from
`midnightntwrk/passport@40072709e3f5ac5d9b89f9d92d02a4413cbe05cc`. It does
not deploy contracts, submit transactions, talk to nodes/indexers/proof
servers, implement capture/wave policy, or generate Compact bindings. Runtime
inbox discovery is deliberately abstract: callers must supply the read-only
chain evidence needed to reject entries whose decrypted plaintext does not match
the deposited coin commitment/qualified coin.

Compatibility: the source contract is reviewed against Ledger 9.1 with
Compact `0.33.0-rc.2` / runtime `0.18.0-rc.1`. The Jubjub arm is the normative
MIP-0013 arm. The k256 arm is interim, non-normative engineering parity for
the upstream revision.


## Provenance

Ported material is derived from the Apache-2.0 upstream Passport repository at
`midnightntwrk/passport@40072709e3f5ac5d9b89f9d92d02a4413cbe05cc`:

- `contract/signer-rs/src/main.rs`
- `contract/signer-rs/src/grants.rs`
- `contract/src/wallet/signer.ts`
- `contract/src/wallet/inbox.ts`
- `contract/src/wallet/witnesses.ts`
- `contract/src/wallet/discovery.ts`
- `contract/src/tests/vectors/grants-e1.json`
- `docs/plans/components/C1-account-custody-contract.md` and `contract/README.md`

The sibling `midnight-passport-account-source` crate preserves the
byte-identical upstream Compact snapshot as `contract/account.upstream.compact`
and exposes `contract/account.compact` with documented downstream device-key
and k256 envelope hardening for activation and public construction helpers.
