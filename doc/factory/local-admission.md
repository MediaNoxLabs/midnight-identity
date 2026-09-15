<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Local delivery admission

[`bootstrap.sh`](../../bootstrap.sh) is the supported local entrypoint. It
discovers the standard Nix daemon profile when the desktop shell does not have
Nix on `PATH`, enters the repository-pinned environment, and keeps factory
commands rooted in the active worktree. A lightweight `factory` shell pins
Node.js 24, GitHub CLI, and Git for policy commands and hook execution; hooks
do not inherit ambient host versions of those tools.

Configure the repository-local contribution hooks once in each clone:

```bash
./bootstrap.sh --configure-git
./bootstrap.sh --check
```

The installer changes only this repository's Git configuration. It selects
the tracked `.githooks` directory, enables commit signing, and pins the
repository-local signing format to OpenPGP; it does not edit
global/user configuration, authentication, signing keys, remotes, or branch
protection. The hooks provide fast feedback for commit-message grammar,
staged whitespace/secrets, and exact-head push admission. Hosted CI repeats
every invariant because local hooks can be bypassed.

## Exact-head L0 receipt

After committing a production-ready candidate, run:

```bash
./bootstrap.sh --local-gate --delivery-target develop
```

For an explicitly stacked candidate, add its temporary diff base:

```bash
./bootstrap.sh --local-gate \
  --base fix/issue-58 \
  --delivery-target develop
```

The gate requires a clean issue-backed worktree and a signed/DCO-valid commit
range. A durable base must match the named delivery target; only an
issue-backed base is accepted as a temporary stack parent. Authored signatures
must contain an OpenPGP commit envelope in addition to Git's successful local
verification. The gate runs the repository factory contracts, contribution policy,
added-line secret scan, and `git diff --check`, then records the immutable
target plan. The private mode-0600 receipt lives below the Git common
directory in `midnight-identity-factory/local-gates-v1`; it is shared by the
clone's linked worktrees but never committed.

Receipt creation fetches the selected branch from `origin` (and the durable
delivery target for a stack) before resolving the base SHA. The pre-push hook
verifies the receipt against the pushed branch/head and queries `origin` for
the current exact base SHA, so an unavailable or advanced remote base fails
closed. Authored commits must verify in the local OpenPGP
keyring. A GitHub update-branch merge is instead checked against GitHub's
verified commit record, because the forge key need not be valid in the local
keyring. A changed head, advanced base, wrong branch, malformed record,
missing/failed check, unavailable ancestry, or unavailable signature evidence
invalidates the receipt. Rerun the gate instead of editing it.

This is local L0 evidence, not a substitute for hosted CI. The receipt records
the planner's affected Rust, WASM, coverage, and Compact targets; GitHub runs
those selected lanes against the pushed exact head and the stable `Required
CI` aggregator remains authoritative.

## Operator commands

```bash
./bootstrap.sh                 # full Compact and Pi development shell
./bootstrap.sh --rust          # light Rust development shell
./bootstrap.sh --check         # factory contracts in the pinned shell
./bootstrap.sh --pi            # audit policy, then start pinned Pi
./bootstrap.sh -- COMMAND ...  # one command in the pinned full shell
```

Restart Pi after changing `.pi/`, `.devloops`, or factory runtime policy; a
running process retains its loaded configuration.
