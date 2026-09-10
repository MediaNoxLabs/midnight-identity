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
   human-only merges.

Everything pi writes locally (`.pi/git/`, `.pi/npm/`, `.pi/harness/`)
is git-ignored; the tracked files under `.pi` are `settings.json`
(CODEOWNERS-gated) and the skill packs in `.pi/skills/` (currently
the OpenSpec change-workflow skills). Together with `.devloops`, they
are repo-owned tooling — review changes to them like executable
code.

## Commands

Inside the pi shell at the repo root:

| Command | Purpose |
|---|---|
| `/dev-loops doctor` | Validate `.devloops` against the schema |
| `/dev-loops gates` | List the review gates for this repo |
| `/dev-loops start <issue>` | Begin a loop from a GitHub issue |
| `/dev-loops status <issue>` | Report loop state |
| `/dev-loops continue <pr>` | Resume on an open PR |

Non-pi fallback (no shell): `npx dev-loops@0.9.0 doctor` / `gates`.

## Boundaries

- pi is the operator interface; it does not merge PRs and does not
  replace branch protection.
- Do not expose an unauthenticated `pi --mode rpc` process as a network
  service.
- Validation commands pi runs are exactly the ones CI runs
  (`nix develop --command just ci`); pi adds no privileged path.
