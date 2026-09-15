#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

if [[ -z "${PACKAGES_CSV:-}" ]]; then
  echo "PACKAGES_CSV must contain at least one selected package" >&2
  exit 1
fi

IFS=, read -r -a packages <<< "$PACKAGES_CSV"
args=()
for package in "${packages[@]}"; do
  args+=(-p "$package")
done

case "${1:-}" in
  check)
    cargo fmt "${args[@]}" -- --check
    cargo clippy --locked "${args[@]}" --all-targets -- -D warnings
    cargo check --locked "${args[@]}" --all-targets
    ;;
  test)
    cargo test --locked "${args[@]}"
    ;;
  *)
    echo "usage: rust-target.sh check|test" >&2
    exit 1
    ;;
esac
