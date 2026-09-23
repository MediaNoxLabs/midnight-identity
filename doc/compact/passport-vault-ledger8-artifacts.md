<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Passport Vault Ledger 8 artifact leaf

Issue #82 adds the upstream, Ledger-8-specific artifact leaf for the Passport
Vault source package. The lightweight `midnight-passport-vault-source` crate
remains the source authority and stays free of Compact compiler, Midnight
Ledger, ZKIR, Halo2/proving-key, parameter, and alternate-ledger dependency
cones.

The committed leaf is the authenticated manifest at
`artifacts/passport-vault-ledger8/manifest.json`. Regenerate it from a clean
checkout with:

```bash
node scripts/compact/passport-vault-ledger8-artifacts.mjs \
  --oxid-reference /path/to/read-only/oxid \
  --out artifacts/passport-vault-ledger8/manifest.json
```

The generator verifies the source contract digest and the transitive include
closure from `midnight-passport-vault-source` before writing the manifest. With
`--oxid-reference`, it also verifies that the read-only Oxid checkout is clean,
that `HEAD` equals `origin/develop`, and records the exact SHA and source path
used for the baseline comparison. Hosted CI runs without a developer-local Oxid
checkout by reading the pinned repo-local baseline file at
`artifacts/passport-vault-ledger8/oxid-baseline.json`; that file records the
same immutable Oxid repository, SHA, derivation path digest, and license
provenance observed before this slice. The manifest records:

- source identity and include-closure digests;
- exact Ledger 8, Compact compiler/CLI, generated runtime, and proof/ZKIR pins;
- ABI/state-vector and public-input identities derived from the authenticated
  source and circuit inventory;
- circuit `k` and row baselines from the reviewed Oxid Ledger 8 cohort;
- deterministic cache-key inputs;
- the pinned Oxid Ledger 8 Nix derivation that materializes the heavy artifact
  set and emits per-artifact digests; and
- the heavy outputs that must be reproducibly derived but never committed.

Two consecutive generator runs must produce byte-identical manifests. The
focused Node test under `tests/compact/` enforces that property and checks that
ZKIR, proving/verifying keys, parameter files, `target/`, and `result` outputs
are not tracked by Git.

## Consumer handoff and rollback

The later Oxid cutover tracked by Oxid #123 must pin an immutable
`midnight-identity` Git revision or release containing this manifest. Oxid must
compare its existing `nix/packages/passport-vault-compact-artifacts.nix` output
against this manifest at the exact consumer head before replacing any local
source or artifact reference. Rollback is the previous Oxid pin plus the
currently recorded Oxid artifact derivation; this slice does not modify Oxid and
does not delete its duplicate source.

## Metrics

This repository slice records generation metadata and cache inputs. Full cold
and warm generation time, generated artifact byte size, cache byte size, and
consumer dependency-closure deltas require the hosted/Nix artifact build and are
therefore recorded as `null` in the manifest until that build materializes the
heavy outputs outside Git.
