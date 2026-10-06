<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# Factory metrics and periodic audit

Each production-ready run posts one readable PR or issue summary followed by
hidden machine-readable JSON conforming to
[`work-item-metrics-v1.schema.json`](work-item-metrics-v1.schema.json). Raw
prompts, transcripts, commands/output, credentials, keys, DIDs,
and private identifiers are forbidden.

## Per-run record

The record binds an issue and optional PR to one exact head and contains:

- model and reasoning profile reported by the active harness;
- elapsed, implementation, review, CI, and retry durations;
- exact non-overlapping token counters when available, otherwise `null`;
- disk delta plus observed peak worktree, target, and process RSS bytes;
- local validations and hosted CI lanes with outcome/duration;
- a bounded failure classification and retry count.

Generate a template outside the checkout, replace unknown required values, and
format it for a PR/issue comment:

```bash
node scripts/factory/metrics.mjs template \
  --issue N --pr N --head "$(git rev-parse HEAD)" \
  --model '<reported-model>' --reasoning '<reported-profile>' \
  > /private/path/metrics.json
node scripts/factory/metrics.mjs validate --file /private/path/metrics.json
node scripts/factory/metrics.mjs format --file /private/path/metrics.json
```

Zero means a measured zero, never an unavailable value. Do not estimate tokens
from transcript text or add child totals when the parent already includes
them. The formatter emits a concise Markdown table and an HTML comment
containing canonical JSON.

## Periodic audit

The scheduled CI audit and a local supervisor may run:

```bash
node scripts/factory/audit.mjs --online --format markdown
```

The audit is read-only. It reports exact/stale pi.dev package pins,
disabled package surfaces, recent CI duration/outcomes and retry causes, cache
inventory, worktree/target disk use, preservation reasons for abandoned-looking
worktrees, and recent merged-PR throughput. Missing GitHub/npm access is
reported as unavailable rather than guessed. Audit findings become bounded
issues; the audit itself never upgrades packages, deletes caches/worktrees,
retries jobs, creates agents, or merges code.
