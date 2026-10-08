#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

ledger_revision=2cce0f8f26e8ab1398af1c9ed61b087c28611a3f
compact_revision=e17b2a42227efa84281c6ce41868b66eb909e236
proofs_revision=532629b044a88473a7175f4a96c2511c91156136
repository_url=https://github.com/MediaNoxLabs/midnight-identity.git
revision=

while (( $# > 0 )); do
  case "$1" in
    --repo-url)
      repository_url=${2:?--repo-url requires a value}
      shift 2
      ;;
    --rev)
      revision=${2:?--rev requires a value}
      shift 2
      ;;
    -h|--help)
      echo "usage: $0 --rev <40-hex-sha> [--repo-url <git-url>]"
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ ! "$revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "--rev must be one immutable 40-character lowercase Git revision" >&2
  exit 2
fi
if [[ "$repository_url" == *'"'* || "$repository_url" == *$'\n'* ]]; then
  echo "--repo-url contains characters that cannot be represented safely in Cargo.toml" >&2
  exit 2
fi

scratch=$(mktemp -d "${TMPDIR:-/tmp}/midnight-did-git-consumer.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/src"
cat > "$scratch/Cargo.toml" <<EOF
[package]
name = "midnight-did-git-consumer"
version = "0.0.0"
edition = "2024"
rust-version = "1.89"
publish = false

[dependencies]
midnight-did-runtime = { git = "$repository_url", rev = "$revision", features = ["http", "node-subxt"] }

# Cargo does not inherit patches from a Git dependency. Ledger8 transient
# crypto requires the maintained proofs fork's disk-spill feature.
[patch.crates-io]
midnight-proofs           = { git = "https://github.com/MediaNoxLabs/midnight-zk.git", rev = "$proofs_revision" }
EOF
printf 'fn main() {}\n' > "$scratch/src/main.rs"

metadata="$scratch/metadata.json"
consumer_target=${GIT_CONSUMER_TARGET_DIR:-$scratch/target}
CARGO_TARGET_DIR="$consumer_target" cargo metadata \
  --format-version 1 \
  --manifest-path "$scratch/Cargo.toml" > "$metadata"

assert_single_source() {
  local package=$1 repository=$2 expected_revision=$3
  local total matching
  total=$(jq --arg package "$package" '[.packages[] | select(.name == $package)] | length' "$metadata")
  matching=$(jq \
    --arg package "$package" \
    --arg repository "$repository" \
    --arg revision "$expected_revision" \
    '[.packages[] | select(.name == $package and (.source // "" | contains($repository)) and (.source // "" | contains($revision)))] | length' \
    "$metadata")
  if [[ "$total" != 1 || "$matching" != 1 ]]; then
    echo "$package did not resolve exactly once from $repository@$expected_revision" >&2
    jq --arg package "$package" '.packages[] | select(.name == $package) | {name, version, source}' "$metadata" >&2
    exit 1
  fi
}

for package in \
  midnight-base-crypto \
  midnight-coin-structure \
  midnight-onchain-runtime \
  midnight-onchain-state \
  midnight-onchain-vm \
  midnight-serialize \
  midnight-storage \
  midnight-transient-crypto \
  midnight-zswap \
  midnight-ledger
do
  assert_single_source "$package" MediaNoxLabs/midnight-ledger "$ledger_revision"
done
assert_single_source midnight-compact-runtime MediaNoxLabs/compact "$compact_revision"
assert_single_source midnight-proofs MediaNoxLabs/midnight-zk "$proofs_revision"

if jq -e '.packages[] | select(.name == "midnight-did-uniffi")' "$metadata" >/dev/null; then
  echo "the supported Git consumer cone must not expose midnight-did-uniffi" >&2
  exit 1
fi

CARGO_TARGET_DIR="$consumer_target" cargo check \
  --manifest-path "$scratch/Cargo.toml"
