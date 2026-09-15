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
families_selected=false
for package in "${packages[@]}"; do
  args+=(-p "$package")
  if [[ "$package" == "midnight-vc-families" ]]; then
    families_selected=true
  fi
done

case "${1:-}" in
  check)
    cargo fmt "${args[@]}" -- --check
    cargo clippy --locked "${args[@]}" --all-targets --no-deps -- -D warnings
    cargo check --locked "${args[@]}" --all-targets
    if [[ "$families_selected" == true ]]; then
      cargo clippy --locked -p midnight-vc-families --all-features --all-targets -- -D warnings
      cargo check --locked -p midnight-vc-families --all-features --all-targets
    fi
    ;;
  test)
    cargo test --locked "${args[@]}"
    if [[ "$families_selected" == true ]]; then
      cargo test --locked -p midnight-vc-families --all-features
    fi
    ;;
  *)
    echo "usage: rust-target.sh check|test" >&2
    exit 1
    ;;
esac
