<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-passport-account-source

Authenticated, byte-identical Compact source and compatibility metadata for the
Midnight Passport account-custody reference contract implementing MIP-0012 and
MIP-0013.

The package is deliberately dependency-light: it contains no Ledger, Compact
runtime, Halo2, ZKIR, generated bindings, or proving artifacts. The embedded
source imports only `CompactStandardLibrary`. Consumers can authenticate it via
`CONTRACT_SHA256` or read `manifest.json` directly.

The descriptor preserves the reviewed Ledger 9.1 toolchain cohort and its known
boundaries. In particular, recovery is specified without an exposed recovery
circuit, the k256 arm is interim engineering rather than normative MIP-0013,
and deployment of the full contract must respect the reviewed Ledger 9 block
limits.
