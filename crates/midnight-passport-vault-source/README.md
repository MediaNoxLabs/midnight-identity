<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-passport-vault-source

Authenticated, byte-identical Compact source for the Passport Vault contract,
its complete vendored credential include closure, a machine-readable
compatibility manifest, and reviewed circuit baselines.

This package is deliberately lightweight: it has no runtime dependencies and
does not contain generated Rust, ZKIR, proving parameters, or proving keys.
Those derivable outputs belong in a separate ledger-specific artifact package.

Rust consumers can authenticate the embedded source using
`CONTRACT_SHA256` and `INCLUDE_CLOSURE_SHA256`; build systems can read
`manifest.json` directly from the published crate or an immutable Git checkout.
