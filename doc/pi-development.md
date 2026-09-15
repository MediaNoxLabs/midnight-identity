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

1. `./bootstrap.sh --pi` provisions the `pi` binary (the `pi-coding-agent`
   package from nixpkgs) in the lightweight factory shell. The full
   `nix develop` shell still includes Pi, Rust, and Compact for explicit
   build/codegen work, but factory startup does not require materializing the
   Compact/Rust closure.
2. Running `pi` at the repo root reads [`.pi/settings.json`](../.pi/settings.json);
   the first session asks you to **trust** the repository because that
   file installs extension packages. It pins `dev-loops` and `pi-subagents`
   (public npm) into the git-ignored `.pi/npm/` prefix, with
   `openai-codex/gpt-5.5` at medium reasoning as the repository default for
   Pi 0.75.4. Oxid currently exercises newer `gpt-5.6-terra` and taskflow/UI
   packages; those are intentionally not adopted here because this Nix-pinned
   Pi runtime does not list that model and the factory needs only one bounded
   supervisor-to-worker delegation path.
3. The `dev-loops` package extension is intentionally disabled in settings
   (`extensions: []`) so Pi does not expose unavailable `/dev-loops` slash
   commands. Use the documented `npx dev-loops@1.0.2 doctor` and
   `npx dev-loops@1.0.2 gates` fallback to validate [`.devloops`](../.devloops)
   and inspect gates. The tracked `factory-supervisor` and non-invocable
   `factory-worker` roles implement the bounded supervisor/sole-writer split;
   delivery and worker limits are machine-readable in `.pi/`.

The `pi-subagents@0.35.1` pin is selected from package metadata, not from
`./bootstrap.sh --pi --version` alone: `npm view pi-subagents@0.35.1
peerDependencies` shows its peer dependencies are wildcards for `@earendil-works/pi-ai`,
`@earendil-works/pi-tui`, `@earendil-works/pi-agent-core`,
`@earendil-works/pi-coding-agent`, and `typebox`, while
`pi-subagents@0.42.1` requires `@earendil-works/pi-ai >=0.80.0`. The external
supervisor still owns the real tracked-extension startup smoke before final
approval.

Everything pi writes locally (`.pi/git/`, `.pi/npm/`, `.pi/harness/`,
`.pi/agent/`, and `.pi/sessions/`) is git-ignored. Settings, delivery profiles,
subagent policy, and repository agents are tracked and CODEOWNERS-gated — review
changes to them like executable tooling.

## Commands

Inside the Pi shell at the repo root:

| Command | Purpose |
|---|---|
| `/factory-supervisor prototype issue <n>` | Run one local provisional issue loop |
| `/factory-supervisor production-ready issue <n>` | Produce and supervise one draft delivery candidate |

`/factory-supervisor` is the supported Pi delivery command. Do not use or
advertise `/dev-loops` slash commands in this repository while the `dev-loops`
extension remains disabled.

Non-Pi fallback for the disabled `dev-loops` surfaces:

```bash
npx dev-loops@1.0.2 doctor
npx dev-loops@1.0.2 gates
```

Outside Pi, `doctor` may report the `subagent` command unavailable until the
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

## Provenance-bound extraction flow

Reusable components may be extracted only from the read-only consumer
repositories listed in [`.pi/extraction-policy.json`](../.pi/extraction-policy.json):
`MediaNoxLabs/oxid` and `input-output-hk/lace-id-portal`. The authorized Portal
checkout origin is `https://github.com/input-output-hk/lace-id-portal.git`; its
remote symbolic HEAD is `refs/heads/develop`, so the recorded reference remains
`origin/develop`. Before admitting the worker, the supervisor records each used
checkout's origin remote, `origin/develop` SHA, and `git status --short` with:

```bash
node scripts/factory/reference-repositories.mjs observe \
  --repository MediaNoxLabs/oxid --path /absolute/path/to/oxid
```

The worker may read only the recorded revisions and must return source
repository, exact SHA, source paths, and license/provenance evidence in the
handoff. The supervisor repeats the observation after the worker and stops if
the remote, exact SHA, or status changed, or if source-path/provenance evidence
is missing. `MediaNoxLabs/midnight-identity` remains the sole mutation target.
