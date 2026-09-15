#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$repo_root"

usage() {
  printf '%s\n' \
    "Usage: ./bootstrap.sh [--rust]" \
    "       ./bootstrap.sh --check" \
    "       ./bootstrap.sh --configure-git" \
    "       ./bootstrap.sh --local-gate [--base REF] --delivery-target TARGET" \
    "       ./bootstrap.sh --verify-local-gate --head SHA --branch BRANCH" \
    "       ./bootstrap.sh --pi [PI_ARGS...]" \
    "       ./bootstrap.sh -- COMMAND [ARGS...]" \
    "" \
    "With no arguments, enter the full Compact/Pi shell. --rust enters the" \
    "light Rust shell. --pi enters the light Pi/factory shell. Factory" \
    "commands and hooks use the pinned policy shell."
}

nix_bin="$(command -v nix || true)"
if [[ -z "$nix_bin" && -x /nix/var/nix/profiles/default/bin/nix ]]; then
  nix_bin=/nix/var/nix/profiles/default/bin/nix
fi
if [[ -z "$nix_bin" ]]; then
  echo "Nix with flakes enabled is required." >&2
  exit 1
fi

run_pinned() {
  local shell_name="$1"
  shift
  "$nix_bin" develop "$shell_name" --command bash -c '
    set -euo pipefail
    repo_root="$1"
    shift
    cd "$repo_root"
    exec "$@"
  ' midnight-identity-bootstrap "$repo_root" "$@"
}

run_factory() {
  run_pinned .#factory "$@"
}

case "${1:-}" in
  "")
    exec "$nix_bin" develop
    ;;
  --rust)
    shift
    if (( $# != 0 )); then usage >&2; exit 2; fi
    exec "$nix_bin" develop .#rust
    ;;
  --check)
    shift
    if (( $# != 0 )); then usage >&2; exit 2; fi
    run_factory node scripts/factory/check.mjs
    ;;
  --configure-git)
    shift
    if (( $# != 0 )); then usage >&2; exit 2; fi
    run_factory node scripts/git-hooks/configure.mjs apply --execute
    ;;
  --local-gate)
    shift
    run_factory node scripts/factory/local-gate.mjs run "$@"
    ;;
  --verify-local-gate)
    shift
    run_factory node scripts/factory/local-gate.mjs verify "$@"
    ;;
  --pi)
    shift
    run_pinned .#pi bash -c '
      set -euo pipefail
      node scripts/factory/check.mjs
      exec pi "$@"
    ' midnight-identity-pi "$@"
    ;;
  --hook)
    shift
    hook_name="${1:-}"
    shift || true
    case "$hook_name" in
      commit-msg|pre-commit|pre-push)
        run_factory node scripts/git-hooks/local-policy.mjs "$hook_name" "$@"
        ;;
      *)
        echo "Unsupported repository hook: $hook_name" >&2
        exit 2
        ;;
    esac
    ;;
  --)
    shift
    if (( $# == 0 )); then usage >&2; exit 2; fi
    run_pinned .#default "$@"
    ;;
  --help|-h)
    usage
    ;;
  *)
    echo "Unknown bootstrap argument: $1" >&2
    usage >&2
    exit 2
    ;;
esac
