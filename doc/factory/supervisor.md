<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Pi supervisor and worktree lifecycle

The tracked Pi roles are [factory supervisor](../../.pi/agents/factory-supervisor.agent.md)
and [factory worker](../../.pi/agents/factory-worker.agent.md). Package pins and
retry settings live in [`.pi/settings.json`](../../.pi/settings.json); delivery
and delegation bounds live beside them. Pi is optional—the Nix/Just/Node
commands remain authoritative outside Pi.

## Supervisor flow

1. Resolve one issue, its explicit delivery target, type, and acceptance
   criteria. Classify reversibility, blast radius, and evidence cost.
2. Run `node scripts/factory/worktree-lifecycle.mjs audit`. Reuse an
   identity-matching issue worktree or create one exact
   `<type>/issue-<number>` worktree from the fetched target.
3. For crystallization/extraction issues, observe every referenced consumer
   checkout named in `.pi/extraction-policy.json` before delegation: repository
   identity, origin remote, `origin/develop` SHA, and status. Consumers are
   read-only; `MediaNoxLabs/midnight-identity` is the only mutation target.
4. Admit exactly one `factory-worker` for the issue. Pass the issue, worktree,
   branch, base, profile, acceptance criteria, selected target plan, and any
   recorded reference-repository observations.
5. The worker edits and validates only in that worktree. It may not spawn a
   child, enter taskflow, detach a retry loop, merge, or change repository
   settings. It returns a draft candidate to the supervisor.
6. The supervisor reviews the diff, repeats any reference-repository
   observations, stops on changed consumer remote/head/status or missing source
   path/license provenance, creates/updates the draft PR when authorized,
   watches exact-head CI, classifies failures, and permits at most one bounded
   retry for a repairable implementation/CI failure. Review triage filters
   status/summary/duplicate/stale automated noise; every actionable finding is
   fixed in the same PR or linked to a newly created follow-up issue. Follow-up
   is only for bounded nonblocking polish. Acceptance, correctness, security,
   provenance, required-test, and CI findings are same-PR fixes, and readiness
   is forbidden while any review thread is unresolved.
7. The supervisor posts the run summary plus hidden metric JSON and hands
   durable-branch merge control to a human.
8. Only after hosted merged-PR evidence exists, run an audited exact-path
   closeout from outside the selected worktree.

One parent session admits only one implementation worker per issue. A separate
reviewer may inspect a clean exact head, but it never shares write ownership.
Provider or transport failures do not create extra agents or infinite retries.

## Lifecycle commands

Audit is read-only:

```bash
node scripts/factory/worktree-lifecycle.mjs audit
```

Closeout is destructive and therefore requires every identity explicitly:

```bash
node scripts/factory/worktree-lifecycle.mjs closeout \
  --pr N \
  --path /absolute/path/to/issue-N \
  --expect-head 40-character-sha \
  --execute
```

The command refuses the current or main checkout; unknown, nested, dirty,
locked, detached, mismatched, active-PR, unmerged, or unreachable merge state;
and ambiguous GitHub evidence. It removes only the exact worktree and its exact
local branch. It never sweeps by glob, branch prefix, age, or remote deletion.
