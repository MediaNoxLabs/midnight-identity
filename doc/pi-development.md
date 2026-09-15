<!--
This file is part of MediaNoxLabs/midnight-identity.
Copyright (C) 2026 Midnight Foundation
SPDX-License-Identifier: Apache-2.0
-->

# pi.dev development shell

This repository supports [pi](https://pi.dev) as an **optional** local
interface for the pinned `dev-loops` development workflow, mirroring the
setup in [midnightntwrk/midnight-did](https://github.com/midnightntwrk/midnight-did)
(`docs/pi-development.md` there). The repository does **not** require pi
to build, test, or review code — `nix develop` + `just` remain the
canonical entry points, and GitHub Actions stays the authoritative
source for PR checks.

## How it layers over nix

The layering direction is: **nix provides pi, pi reads the repo policy.**

1. `nix develop` provisions the `pi` binary (the `pi-coding-agent`
   package from nixpkgs) alongside the Rust toolchain.
2. Running `pi` at the repo root reads [`.pi/settings.json`](../.pi/settings.json);
   the first session asks you to **trust** the repository because that
   file installs extension packages. It pins `dev-loops` (public npm)
   into the git-ignored `.pi/npm/` prefix.
3. The `/dev-loops` slash-commands enforce the repo policy in
   [`.devloops`](../.devloops) — gates, validation commands (`just`
   recipes under `nix develop`), signed+DCO commit requirements, and
   human-only merges. The tracked `factory-supervisor` and non-invocable
   `factory-worker` roles implement the bounded supervisor/sole-writer split;
   delivery and worker limits are machine-readable in `.pi/`.

Everything pi writes locally (`.pi/git/`, `.pi/npm/`, `.pi/harness/`,
`.pi/agent/`, and `.pi/sessions/`) is git-ignored. Settings, delivery profiles,
subagent policy, and repository agents are tracked and CODEOWNERS-gated — review
changes to them like executable tooling.

## Commands

Inside the pi shell at the repo root:

| Command | Purpose |
|---|---|
| `/dev-loops doctor` | Validate `.devloops` against the schema |
| `/dev-loops gates` | List the review gates for this repo |
| `/dev-loops start <issue>` | Begin a loop from a GitHub issue |
| `/dev-loops status <issue>` | Report loop state |
| `/dev-loops continue <pr>` | Resume on an open PR |
| `/factory-supervisor prototype issue <n>` | Run one local provisional issue loop |
| `/factory-supervisor production-ready issue <n>` | Produce and supervise one draft delivery candidate |

Non-pi fallback (no shell): `npx dev-loops@0.9.0 doctor` / `gates`.
Outside Pi, `doctor` reports the `subagent` command unavailable until the
tracked `pi-subagents` package is loaded by Pi; `gates` remains the standalone
configuration check.

## Boundaries

- pi is the operator interface; it does not merge PRs and does not
  replace branch protection.
- One supervisor admits one implementation worker per issue. The worker cannot
  delegate; retries are bounded to one and taskflow/detached retry loops are
  outside the supported path.
- Do not expose an unauthenticated `pi --mode rpc` process as a network
  service.
- Use `node scripts/factory/check.mjs` for harness/configuration changes and the immutable
  target plan for code. Pi adds no privileged validation or merge path.
