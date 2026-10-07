#!/usr/bin/env bash
# This file is part of MediaNoxLabs/midnight-identity.
# Copyright (C) 2026 Midnight Foundation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

root=$(git rev-parse --show-toplevel)
if [[ "$root" == *"'"* ]]; then
  echo "repository path cannot contain a single quote: $root" >&2
  exit 2
fi

ledger_root="$root/third_party/midnight-ledger"
compact_root="$root/third_party/compact"
ledger_crates=(
  midnight-base-crypto:base-crypto
  midnight-coin-structure:coin-structure
  midnight-onchain-runtime:onchain-runtime
  midnight-onchain-state:onchain-state
  midnight-onchain-vm:onchain-vm
  midnight-serialize:serialize
  midnight-storage:storage
  midnight-transient-crypto:transient-crypto
  midnight-zkir:zkir
  midnight-zswap:zswap
  midnight-ledger:ledger
)

for required in "$ledger_root/Cargo.toml" "$compact_root/runtime-rs/Cargo.toml" "$compact_root/runtime-rs-macros/Cargo.toml"; do
  if [[ ! -f "$required" ]]; then
    echo "missing Nix-materialised source: $required" >&2
    echo "enter 'nix develop .#rust' before using this optional wrapper" >&2
    exit 2
  fi
done

cargo_config=()
for entry in "${ledger_crates[@]}"; do
  crate=${entry%%:*}
  relative=${entry#*:}
  path="$ledger_root/$relative"
  cargo_config+=(
    --config "patch.crates-io.$crate.path='$path'"
    --config "patch.\"https://github.com/MediaNoxLabs/midnight-ledger.git\".$crate.path='$path'"
  )
done
cargo_config+=(
  --config "patch.\"https://github.com/MediaNoxLabs/compact.git\".midnight-compact-runtime.path='$compact_root/runtime-rs'"
)

exec cargo "${cargo_config[@]}" "$@"
