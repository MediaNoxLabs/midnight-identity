<!-- Target branch: `develop` (integration). Release-promotion PRs target
     `rust-codegen` (stable mainline). -->

## Overview

<!-- What does this PR change and why? Link the driving issue. -->

## Submission checklist

- [ ] Description explains the what and the why
- [ ] Tests added/updated for behavior changes
- [ ] `nix develop -c just ci` passes locally
- [ ] Commits are DCO signed-off (`git commit -s`) and signed (`-S`, GPG or SSH)
- [ ] `CHANGELOG.md` updated under `[Unreleased]` (user-visible changes)
- [ ] Docs/README updated where behavior or interfaces changed
- [ ] No new TODOs without a tracking issue

## Contract-surface checklist (only if `crates/midnight-did-runtime/src/contract/` changed)

- [ ] `generated.rs` was produced by `just codegen` (no hand edits)
- [ ] `just codegen-check` passes (no drift against the flake-pinned compactc)
- [ ] compact input pin change is called out in the description

## Links

<!-- Closes #NNN -->
