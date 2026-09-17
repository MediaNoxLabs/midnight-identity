<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# midnight-passport-account-source

Authenticated Compact source and compatibility metadata for the Midnight Passport
account-custody reference contract implementing MIP-0012 and MIP-0013.

`contract/account.upstream.compact` is the byte-identical Apache-2.0 snapshot
from `midnightntwrk/passport@40072709e3f5ac5d9b89f9d92d02a4413cbe05cc`.
`contract/account.compact` applies auditable downstream hardening patches on
that snapshot: public k256 construction helpers reject unsupported envelope ids
before creating boot commitments, device entries, or grant identities, and k256
initial activation rejects unsupported boot envelope ids before registering the
first device.

The package is deliberately dependency-light: it contains no Ledger, Compact
runtime, Halo2, ZKIR, generated bindings, or proving artifacts. The embedded
source imports only `CompactStandardLibrary`. Consumers can authenticate the
hardened source via `CONTRACT_SHA256`, authenticate the pristine upstream source
via `UPSTREAM_CONTRACT_SHA256`, or read `manifest.json` directly.

The descriptor preserves the reviewed Ledger 9.1 toolchain cohort and its known
boundaries. In particular, recovery is specified without an exposed recovery
circuit, the k256 arm is interim engineering rather than normative MIP-0013,
and deployment of the full contract must respect the reviewed Ledger 9 block
limits.
