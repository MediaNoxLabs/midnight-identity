# AGENT

This is the always-loaded operating contract for `MediaNoxLabs/midnight-identity`,
the repository being evolved as **midnight-libs**. Keep this file concise. Load
the [factory reference](doc/factory/README.md),
[architecture](doc/architecture.md), and relevant
[ADRs](doc/adr/) only when the task touches them.

## Repository boundary

This repository owns reusable Midnight-specific libraries: DID crates and
runtime, reusable VC components, Compact source plus reproducible artifacts,
Midnight bindings and test utilities, and runtime components with at least two
real consumers. Oxid and lace-id-portal are consumers, not folders of this
repository.

Keep application UI, product policy, custody orchestration, deployment
profiles, Tailscale/Tailnet configuration, and demo-specific behavior in their owning
applications. Chain-neutral protocols and primitives belong in `sdk-rust`.
Do not edit consumer repositories unless the owner separately authorizes it.

## Delivery authority

- Resolve and fetch the explicit delivery base. Factory, harness, CI,
  documentation, dependency, and governance work targets `develop`; release
  promotion targets `rust-codegen`. Never infer a base from GitHub's current
  default branch.
- Use one isolated worktree per issue. Name its branch
  `<type>/issue-<number>`, where type is one of `feat`, `fix`, `docs`,
  `refactor`, `test`, `ci`, or `chore`.
- Start pull requests as drafts. An explicitly recorded stacked parent may be
  the temporary PR base; retain `develop` as the final delivery target.
- Every commit and pull-request title follows the approved Conventional Commit
  scopes in [CONTRIBUTING.md](CONTRIBUTING.md). Every commit has an exact DCO
  trailer and a GitHub-verifiable OpenPGP signature.
- Agents never merge to `develop` or `rust-codegen`, rename the repository,
  change credentials/settings/protection/security policy, or publish a release.
  A future guarded merge may target only an owner-approved
  `milestone-x.y.z` policy.

## Architecture, security, and provenance

- Preserve dependency direction: data-model crates remain runtime-neutral;
  Compact-generated modules are never hand-edited; application adapters do not
  leak into library crates.
- Do not invent protocol behavior, cryptographic claims, ledger freshness, or
  custody guarantees. Simulated, cached, indexer-supplied, and proven/live
  states stay distinguishable.
- Never expose secrets, seeds, private keys, witnesses, credentials, or
  sensitive identifiers. Keep actions pinned, permissions least-privilege,
  dependency revisions immutable, and generated artifact provenance explicit.
- New behavior includes focused tests and public contract documentation. A
  green aggregate never hides a skipped affected gate.

## Productive loop

Use exactly one public profile:

- `/dev-loop prototype issue <n>`: local, provisional, narrow affected checks,
  at most one reviewer, no push/PR/hosted-CI wait, and no readiness claim.
- `/dev-loop production-ready issue <n>`: issue worktree, affected-target plan,
  draft PR, one bounded review round, exact-head CI, metrics, and human handoff.
  This is the default when the profile is omitted.

Routine work targets 70% polish. Acceptance criteria, correctness, security,
provenance, required tests, DCO/GPG, and required CI remain 100% mandatory.
Record bounded non-blocking polish as a follow-up issue instead of expanding a
reviewable change.

Before adding process, classify reversibility, blast radius, and evidence cost.
Implement reversible local configuration directly. Do not add canaries,
databases, services, approval stages, or ADRs without a concrete irreversible,
security, protocol, data, or cross-system decision.

The Pi supervisor admits one implementation worker for one issue and then
regains control for review, CI, metrics, and cleanup. Workers cannot delegate.
No nested taskflow, detached retry loop, or uncontrolled retry is allowed.

## Validation and lifecycle

Run the target planner before broad checks:

```bash
node scripts/ci/target-plan.mjs --base origin/develop --head HEAD
```

Use the repository Nix shell for Rust and Compact targets. `prototype` runs the
narrowest meaningful affected target. `production-ready` runs every selected
correctness, security, provenance, generation, and compatibility target. Run
`npx dev-loops@0.9.0 doctor`, `npx dev-loops@0.9.0 gates`, and
`node scripts/factory/check.mjs` after changing factory or Pi configuration.

Before pushing, verify the intended diff, clean status, commit convention,
DCO trailer, good GPG signature, selected local gates, no personal config or
credentials, and no unexpected generated artifacts.

Audit worktrees before creating or closing one. Cleanup requires one exact
path, expected head SHA, and merged-PR evidence. Unknown, dirty, locked,
active, mismatched, or unmerged worktrees are preserved.
