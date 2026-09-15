<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Delivery contract

## Issue and branch

Every implementation begins with one problem-statement issue containing a
goal, acceptance criteria, non-goals, delivery target, and focused validation.
One implementation worker owns one isolated worktree and one branch:

```text
feat/issue-N     fix/issue-N      docs/issue-N
refactor/issue-N test/issue-N     ci/issue-N
chore/issue-N
```

Factory, harness, CI, documentation, dependency, and governance items target
`develop`. Release promotion targets `rust-codegen`. A stacked PR can
temporarily target its issue-backed parent branch, but its issue and handoff
must retain the final target. The GitHub default branch is not delivery
authority. The PR template's single `factory-delivery-target` and
`factory-stacked-parent` markers are the hosted machine-readable record. A
direct PR records `none`; a stack records the exact `<type>/issue-N` parent.

## Profiles

The machine-readable contract is [`.pi/delivery-profiles.json`](../../.pi/delivery-profiles.json).

`prototype` is a reversible local hypothesis loop. It preserves issue,
worktree, branch, security, process, and disk invariants; selects only the
basic factory contract plus directly affected checks; permits at most one
reviewer and one bounded retry; and cannot push, open/update a PR, wait for
hosted CI, merge, or claim production readiness.

`production-ready` is the default. It refreshes the recorded base, invalidates
prototype evidence, recomputes affected targets, creates a draft PR, runs one
bounded review round, supervises exact-head hosted CI, posts metrics, and
hands `develop` or `rust-codegen` merge control to a human.

Both profiles require complete acceptance, correctness, security, provenance,
tests selected by the planner, DCO, GPG, and process/disk invariants. The 70%
quality budget applies only to polish beyond those invariants.

## Contribution evidence

Before each push, verify the full local commit range:

```bash
git log --format='%h %G? %s%n%(trailers:key=Signed-off-by)' <base>..HEAD
git diff --check <base>...HEAD
git status --short
```

Every commit must have a good signature (`G`), one exact `Signed-off-by`
trailer matching the author, and an approved Conventional Commit title/scope.
Hosted contribution policy additionally checks GitHub's signature verification
for each exact PR commit. Human squash merge remains supported: the resulting
single-parent commit is exempt from an authored DCO trailer only when GitHub is
the committer, the signature is GitHub-verified, and the canonical ` (#N)`
subject suffix follows a valid scoped PR title. Ordinary authored commits are
never covered by that exception.

## Draft and handoff

The implementation worker stops after producing a focused draft candidate and
returns the issue, PR, exact head, changed paths, selected checks, durations,
resource observations, and known risks to the supervisor. The supervisor owns
review triage, CI, retry classification, metrics, and eventual closeout. It
does not merge durable branches.
